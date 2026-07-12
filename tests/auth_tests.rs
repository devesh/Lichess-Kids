use axum::{
    body::{to_bytes, Body},
    http::{header::COOKIE, Request, StatusCode},
    routing::{get, post},
    Router,
};
use lichesskids::db;
use lichesskids::routes::{self, AppState};
use std::sync::{Arc, Mutex};
use tower::ServiceExt;

/// Build the auth-relevant subset of the app with an in-memory DB and a shared
/// handle to that same DB so tests can seed users/tokens.
fn build_app() -> (Router, Arc<Mutex<rusqlite::Connection>>) {
    let conn = db::init_db(":memory:").expect("init db");
    let db_shared = Arc::new(Mutex::new(conn));
    let assets = lichesskids::assets::AssetCatalog::load_from_dir("assets")
        .expect("load assets");
    let state = AppState {
        db: db_shared.clone(),
        lichess_client_id: "test-client".to_string(),
        assets: Arc::new(assets),
    };

    let app = Router::new()
        .route("/api/profile", get(routes::get_profile).delete(routes::delete_profile))
        .route("/api/logout", post(routes::logout))
        .route("/api/oauth/start", post(routes::oauth_start))
        .route("/api/claim-sync", post(routes::claim_sync))
        .route("/api/friends", get(routes::get_friends))
        .route("/api/shop", get(routes::get_shop))
        .with_state(state);

    (app, db_shared)
}

async fn request(app: &Router, method: &str, uri: &str, cookie: Option<&str>) -> StatusCode {
    let mut builder = Request::builder().method(method).uri(uri);
    if let Some(c) = cookie {
        builder = builder.header(COOKIE, c);
    }
    let res = app
        .clone()
        .oneshot(builder.body(Body::empty()).unwrap())
        .await
        .unwrap();
    res.status()
}

// -------------------- Authorization (gating) --------------------

#[tokio::test]
async fn profile_requires_login() {
    let (app, _db) = build_app();
    assert_eq!(
        request(&app, "GET", "/api/profile", None).await,
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn claim_sync_requires_login() {
    let (app, _db) = build_app();
    assert_eq!(
        request(&app, "POST", "/api/claim-sync", None).await,
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn friends_requires_login() {
    let (app, _db) = build_app();
    assert_eq!(
        request(&app, "GET", "/api/friends", None).await,
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn shop_requires_login() {
    let (app, _db) = build_app();
    assert_eq!(
        request(&app, "GET", "/api/shop", None).await,
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn profile_allowed_when_logged_in() {
    let (app, db) = build_app();
    {
        let conn = db.lock().unwrap();
        db::create_user(&conn, "alice", "cat").unwrap();
    }
    assert_eq!(
        request(&app, "GET", "/api/profile", Some("username=alice")).await,
        StatusCode::OK
    );
}

// -------------------- Authentication (token lifecycle) --------------------

#[tokio::test]
async fn logout_clears_stored_token_and_cookie() {
    let (app, db) = build_app();
    {
        let conn = db.lock().unwrap();
        db::create_user(&conn, "alice", "cat").unwrap();
        db::store_lichess_token(&conn, "alice", "tok123", 0).unwrap();
    }

    // Confirm the token is present before logout.
    {
        let conn = db.lock().unwrap();
        assert!(db::get_lichess_token(&conn, "alice").unwrap().is_some());
    }

    // Log out with the session cookie.
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/logout")
                .header(COOKIE, "username=alice")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::OK);

    // Token must be gone and the cookie cleared (Max-Age=0).
    {
        let conn = db.lock().unwrap();
        assert!(db::get_lichess_token(&conn, "alice").unwrap().is_none());
    }
    let set_cookie = res
        .headers()
        .get("set-cookie")
        .expect("set-cookie header")
        .to_str()
        .unwrap();
    assert!(set_cookie.contains("username="));
    assert!(set_cookie.contains("Max-Age=0"));
}

// -------------------- Re-authorization on 401 --------------------

#[tokio::test]
async fn oauth_start_redirect_sends_user_to_lichess() {
    let (app, _db) = build_app();
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/oauth/start?redirect=1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let location = res
        .headers()
        .get("location")
        .expect("location header")
        .to_str()
        .unwrap();
    assert!(location.contains("lichess.org/oauth"));
}

#[tokio::test]
async fn claim_sync_expired_token_signals_reauthorize() {
    let (app, db) = build_app();
    // Logged in (cookie) but no valid Lichess token -> 401 with reauthorize signal.
    {
        let conn = db.lock().unwrap();
        db::create_user(&conn, "alice", "cat").unwrap();
    }
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/claim-sync")
                .header(COOKIE, "username=alice")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    let bytes = to_bytes(res.into_body(), usize::MAX).await.unwrap();
    let line = String::from_utf8(bytes.to_vec()).unwrap();
    let value: serde_json::Value = serde_json::from_str(line.trim()).unwrap();
    assert_eq!(value["reauthorize"], serde_json::json!(true));
    assert!(value["reauthorize_url"]
        .as_str()
        .unwrap()
        .contains("oauth/start"));
}

#[tokio::test]
async fn friends_expired_token_signals_reauthorize() {
    let (app, db) = build_app();
    // Logged in (cookie) but no valid Lichess token -> 401 with reauthorize signal.
    {
        let conn = db.lock().unwrap();
        db::create_user(&conn, "alice", "cat").unwrap();
    }
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/friends")
                .header(COOKIE, "username=alice")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    let bytes = to_bytes(res.into_body(), usize::MAX).await.unwrap();
    let value: serde_json::Value = serde_json::from_str(&String::from_utf8(bytes.to_vec()).unwrap()).unwrap();
    assert_eq!(value["reauthorize"], serde_json::json!(true));
    assert!(value["reauthorize_url"]
        .as_str()
        .unwrap()
        .contains("oauth/start"));
}
