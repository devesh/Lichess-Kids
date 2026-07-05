use axum::{
    routing::{get, post},
    Router,
};
use std::env;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use tower_http::cors::CorsLayer;
use tower_http::services::ServeDir;

use lichesskids::db;
use lichesskids::routes::{
    add_friend, buy_item, claim_sync, delete_friend, delete_profile, equip_item, get_friends, get_profile,
    get_shop, logout, oauth_callback, oauth_start, select_avatar, spin_wheel,
    get_user_profile_html, get_avatar_svg, get_assets_catalog, AppState,
};

#[tokio::main]
async fn main() {
    // 1. Initialize SQLite Database
    let db_path = env::var("DATABASE_URL").unwrap_or_else(|_| "lichesskids.db".to_string());
    println!("Initializing database at: {}", db_path);
    let conn = db::init_db(&db_path).expect("Failed to initialize database");
    let db_shared = Arc::new(Mutex::new(conn));

    // 2. Fetch configurations
    let lichess_client_id = env::var("LICHESS_CLIENT_ID").unwrap_or_else(|_| "lichesskids".to_string());

    println!("Loading assets from directory...");
    let assets = lichesskids::assets::AssetCatalog::load_from_dir("assets")
        .expect("Failed to load assets from directory");
    let assets_shared = Arc::new(assets);

    let state = AppState {
        db: db_shared,
        lichess_client_id,
        assets: assets_shared,
    };

    // 3. Create Router
    let app = Router::new()
        // Server-rendered profiles and SVG generator
        .route("/user/:username", get(get_user_profile_html))
        .route("/api/avatar-svg/:username", get(get_avatar_svg))
        // API Routes
        .route("/api/profile", get(get_profile).delete(delete_profile))
        .route("/api/logout", post(logout))
        .route("/api/oauth/start", post(oauth_start))
        .route("/api/oauth/callback", get(oauth_callback))
        .route("/api/select-avatar", post(select_avatar))
        .route("/api/claim-sync", post(claim_sync))
        .route("/api/spin", post(spin_wheel))
        .route("/api/shop", get(get_shop))
        .route("/api/buy", post(buy_item))
        .route("/api/equip", post(equip_item))
        .route("/api/friends", get(get_friends))
        .route("/api/friends/add", post(add_friend))
        .route("/api/friends/delete", post(delete_friend))
        .route("/api/assets/catalog", get(get_assets_catalog))
        // Serve static assets
        .nest_service("/static", ServeDir::new("static"))
        .fallback_service(ServeDir::new("static"))
        .with_state(state)
        .layer(CorsLayer::permissive());

    // 4. Run Server
    let port = env::var("PORT").unwrap_or_else(|_| "64355".to_string());
    let addr_str = format!("0.0.0.0:{}", port);
    let addr: SocketAddr = addr_str.parse().expect("Invalid bind address");

    println!("LichessKids server running on http://localhost:{}", port);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
