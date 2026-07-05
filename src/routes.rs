use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{Html, IntoResponse, Redirect, Response},
    Json,
};
use base64::Engine;
use rand::Rng;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

use crate::db::{self, EquippedItems, UserProfile};
use crate::lichess;

#[derive(Clone)]
pub struct AppState {
    pub db: Arc<Mutex<Connection>>,
    pub lichess_client_id: String,
    pub redirect_uri: String,
}

// Helper to extract username from cookies
fn get_username(headers: &HeaderMap) -> Option<String> {
    headers
        .get("cookie")
        .and_then(|c| c.to_str().ok())
        .and_then(|cookies_str| {
            cookies_str
                .split(';')
                .map(|s| s.trim())
                .find(|s| s.starts_with("username="))
                .map(|s| s["username=".len()..].to_string())
        })
}

// -------------------------------------------------------------
// USER PROFILE & LOGIN ROUTES
// -------------------------------------------------------------

#[derive(Serialize)]
pub struct ProfileResponse {
    pub profile: UserProfile,
    pub equipped: EquippedItems,
    pub inventory: Vec<String>,
}

pub async fn get_profile(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let username = match get_username(&headers) {
        Some(name) => name,
        None => {
            return Err((
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({ "error": "Not logged in" })),
            ))
        }
    };

    let conn = state.db.lock().unwrap();
    let user_profile = match db::get_user(&conn, &username) {
        Ok(Some(u)) => u,
        Ok(None) => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({ "error": "User profile not found" })),
            ))
        }
        Err(e) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            ))
        }
    };

    let equipped = db::get_equipped(&conn, &username).unwrap_or_default();
    let inventory = db::get_inventory(&conn, &username).unwrap_or_default();

    Ok(Json(ProfileResponse {
        profile: user_profile,
        equipped,
        inventory,
    }))
}

#[derive(Deserialize)]
pub struct MockLoginRequest {
    pub username: String,
    pub avatar_base: Option<String>,
}

pub async fn mock_login(
    State(state): State<AppState>,
    Json(payload): Json<MockLoginRequest>,
) -> impl IntoResponse {
    let username = payload.username.trim();
    if username.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "Username cannot be empty" })),
        )
            .into_response();
    }

    let conn = state.db.lock().unwrap();
    match db::get_user(&conn, username) {
        Ok(Some(_)) => {
            // Already exists, just log in by setting cookie
            let cookie = format!("username={}; Path=/; HttpOnly; Max-Age=86400; SameSite=Lax", username);
            let mut headers = HeaderMap::new();
            headers.insert("set-cookie", cookie.parse().unwrap());
            (
                StatusCode::OK,
                headers,
                Json(serde_json::json!({ "success": true, "registered": true, "username": username })),
            )
                .into_response()
        }
        Ok(None) => {
            // New user, needs avatar selection
            if let Some(avatar) = payload.avatar_base {
                if !vec!["kid_boy", "kid_girl", "cat", "dog", "alien"].contains(&avatar.as_str()) {
                    return (
                        StatusCode::BAD_REQUEST,
                        Json(serde_json::json!({ "error": "Invalid avatar base" })),
                    )
                        .into_response();
                }

                if let Err(e) = db::create_user(&conn, username, &avatar) {
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(serde_json::json!({ "error": e.to_string() })),
                    )
                        .into_response();
                }

                let cookie = format!("username={}; Path=/; HttpOnly; Max-Age=86400; SameSite=Lax", username);
                let mut headers = HeaderMap::new();
                headers.insert("set-cookie", cookie.parse().unwrap());
                (
                    StatusCode::OK,
                    headers,
                    Json(serde_json::json!({ "success": true, "registered": true, "username": username })),
                )
                    .into_response()
            } else {
                // Return success but indicate avatar selection is required
                (
                    StatusCode::OK,
                    Json(serde_json::json!({ "success": true, "registered": false, "username": username })),
                )
                    .into_response()
            }
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

pub async fn logout() -> impl IntoResponse {
    let cookie = "username=; Path=/; HttpOnly; Max-Age=0; SameSite=Lax";
    let mut headers = HeaderMap::new();
    headers.insert("set-cookie", cookie.parse().unwrap());
    (StatusCode::OK, headers, Json(serde_json::json!({ "success": true })))
}

// -------------------------------------------------------------
// OAUTH2 FLOW
// -------------------------------------------------------------

#[derive(Deserialize)]
pub struct OAuthCallbackQuery {
    pub code: String,
    pub state: String,
}

pub async fn oauth_start(State(state): State<AppState>) -> impl IntoResponse {
    let mut rng = rand::thread_rng();
    
    // Generate state and code verifier
    let state_val: String = (0..16).map(|_| rng.sample(rand::distributions::Alphanumeric) as char).collect();
    let code_verifier: String = (0..43).map(|_| rng.sample(rand::distributions::Alphanumeric) as char).collect();

    // Store verifier associated with state
    {
        let conn = state.db.lock().unwrap();
        let _ = conn.execute(
            "CREATE TABLE IF NOT EXISTS oauth_states (
                state TEXT PRIMARY KEY,
                code_verifier TEXT NOT NULL,
                created_at INTEGER NOT NULL
            );",
            [],
        );
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let _ = conn.execute(
            "INSERT INTO oauth_states (state, code_verifier, created_at) VALUES (?1, ?2, ?3)",
            params![state_val, code_verifier, now],
        );
    }

    // Hash verifier for challenge (SHA-256)
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(code_verifier.as_bytes());
    let hash = hasher.finalize();
    
    let code_challenge = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(hash);

    let auth_url = format!(
        "https://lichess.org/oauth?response_type=code&client_id={}&redirect_uri={}&scope=puzzle:read&state={}&code_challenge={}&code_challenge_method=S256",
        state.lichess_client_id,
        urlencoding::encode(&state.redirect_uri),
        state_val,
        code_challenge
    );

    Json(serde_json::json!({ "auth_url": auth_url }))
}

#[axum::debug_handler]
pub async fn oauth_callback(
    State(state): State<AppState>,
    Query(query): Query<OAuthCallbackQuery>,
) -> Response {
    // Retrieve verifier inside a block so we release the DB lock before the network call
    let code_verifier: String = {
        let conn = state.db.lock().unwrap();
        match conn.query_row(
            "SELECT code_verifier FROM oauth_states WHERE state = ?1",
            params![query.state],
            |row| row.get(0),
        ) {
            Ok(v) => {
                let _ = conn.execute("DELETE FROM oauth_states WHERE state = ?1", params![query.state]);
                v
            }
            Err(_) => {
                return Redirect::to("/login.html?error=invalid_state").into_response();
            }
        }
    };

    // Exchange code for token
    let client = reqwest::Client::new();
    let params = [
        ("grant_type", "authorization_code"),
        ("code", &query.code),
        ("redirect_uri", &state.redirect_uri),
        ("client_id", &state.lichess_client_id),
        ("code_verifier", &code_verifier),
    ];

    let token_res = match client
        .post("https://lichess.org/api/token")
        .form(&params)
        .send()
        .await
    {
        Ok(res) => res,
        Err(_) => return Redirect::to("/login.html?error=token_request_failed").into_response(),
    };

    #[derive(Deserialize)]
    struct TokenResponse {
        access_token: String,
    }

    let token_data = match token_res.json::<TokenResponse>().await {
        Ok(data) => data,
        Err(_) => return Redirect::to("/login.html?error=token_parse_failed").into_response(),
    };

    // Fetch user profile from Lichess
    let lichess_profile = match lichess::fetch_profile(&token_data.access_token).await {
        Ok(p) => p,
        Err(_) => return Redirect::to("/login.html?error=profile_fetch_failed").into_response(),
    };

    let username = lichess_profile.username;
    let game_rating = lichess_profile.perfs.blitz.or(lichess_profile.perfs.rapid).map(|p| p.rating).unwrap_or(1500);
    let puzzle_rating = lichess_profile.perfs.puzzle.map(|p| p.rating).unwrap_or(1500);

    // Save token or check if user exists, using a block to drop the lock before returning
    let (exists, cookie) = {
        let conn = state.db.lock().unwrap();
        let _ = conn.execute(
            "CREATE TABLE IF NOT EXISTS lichess_tokens (
                username TEXT PRIMARY KEY,
                access_token TEXT NOT NULL
            );",
            [],
        );
        let _ = conn.execute(
            "INSERT OR REPLACE INTO lichess_tokens (username, access_token) VALUES (?1, ?2)",
            params![username, token_data.access_token],
        );

        let exists = db::get_user(&conn, &username).unwrap().is_some();
        let cookie = format!("username={}; Path=/; HttpOnly; Max-Age=86400; SameSite=Lax", username);

        if exists {
            let _ = db::update_user_ratings(&conn, &username, game_rating, puzzle_rating);
        } else {
            // Create user placeholder, ratings will be set on avatar select
            let _ = conn.execute(
                "INSERT OR IGNORE INTO users (username, avatar_base, current_game_rating, current_puzzle_rating) VALUES (?1, 'kid_boy', ?2, ?3)",
                params![username, game_rating, puzzle_rating],
            );
            let _ = conn.execute("INSERT OR IGNORE INTO equipped (username) VALUES (?1)", params![username]);
        }
        (exists, cookie)
    };

    let mut headers = HeaderMap::new();
    headers.insert("set-cookie", cookie.parse().unwrap());
    if exists {
        headers.insert("Location", "/dashboard.html".parse().unwrap());
    } else {
        headers.insert("Location", "/select-avatar.html".parse().unwrap());
    }
    (StatusCode::SEE_OTHER, headers).into_response()
}

#[derive(Deserialize)]
pub struct SelectAvatarRequest {
    pub avatar_base: String,
}

pub async fn select_avatar(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<SelectAvatarRequest>,
) -> impl IntoResponse {
    let username = match get_username(&headers) {
        Some(name) => name,
        None => return (StatusCode::UNAUTHORIZED, "Not logged in").into_response(),
    };

    if !vec!["kid_boy", "kid_girl", "cat", "dog", "alien"].contains(&payload.avatar_base.as_str()) {
        return (StatusCode::BAD_REQUEST, "Invalid avatar base").into_response();
    }

    let conn = state.db.lock().unwrap();
    let res = conn.execute(
        "UPDATE users SET avatar_base = ?2 WHERE username = ?1",
        params![username, payload.avatar_base],
    );

    match res {
        Ok(_) => (StatusCode::OK, Json(serde_json::json!({ "success": true }))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

// -------------------------------------------------------------
// CLUES & SYNC GAME/PUZZLE ACHIEVEMENTS
// -------------------------------------------------------------

#[axum::debug_handler]
pub async fn claim_sync(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Response {
    let username = match get_username(&headers) {
        Some(name) => name,
        None => return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({ "error": "Not logged in" }))).into_response(),
    };

    let (token, mut puzzle_rating) = {
        let conn = state.db.lock().unwrap();
        let token: Option<String> = conn
            .query_row(
                "SELECT access_token FROM lichess_tokens WHERE username = ?1",
                params![username],
                |row| row.get(0),
            )
            .ok();

        let mut p_rate = 1500;
        if let Some(user) = db::get_user(&conn, &username).unwrap() {
            p_rate = user.current_puzzle_rating;
        }
        (token, p_rate)
    };

    // 1. Fetch user's rating profile
    if let Some(ref t) = token {
        if let Ok(p) = lichess::fetch_profile(t).await {
            let g_rate = p.perfs.blitz.or(p.perfs.rapid).map(|x| x.rating).unwrap_or(1500);
            let p_rate = p.perfs.puzzle.map(|x| x.rating).unwrap_or(1500);
            let conn = state.db.lock().unwrap();
            let _ = db::update_user_ratings(&conn, &username, g_rate, p_rate);
            puzzle_rating = p_rate;
        }
    }

    // 2. Fetch and evaluate games
    // A spin for every person/bot you beat with rating >= user's rating at time of play
    let games = match lichess::fetch_games(&username, token.as_deref(), 30).await {
        Ok(g) => g,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": format!("Games fetch failed: {}", e) }))).into_response(),
    };

    let mut games_claimed = 0;
    {
        let conn = state.db.lock().unwrap();
        for game in games {
            // Is it rated?
            if !game.rated {
                continue;
            }

            // Did the user win?
            let user_won = match game.winner.as_deref() {
                Some("white") => game.players.white.user.as_ref().map(|u| u.id == username.to_lowercase()).unwrap_or(false),
                Some("black") => game.players.black.user.as_ref().map(|u| u.id == username.to_lowercase()).unwrap_or(false),
                _ => false,
            };

            if !user_won {
                continue;
            }

            // Check ratings at game time
            let (user_game_rating, opp_game_rating) = if game.players.white.user.as_ref().map(|u| u.id == username.to_lowercase()).unwrap_or(false) {
                (game.players.white.rating, game.players.black.rating)
            } else {
                (game.players.black.rating, game.players.white.rating)
            };

            if let (Some(u_rate), Some(o_rate)) = (user_game_rating, opp_game_rating) {
                // "beat opponent with rating >= your rating at the time"
                if o_rate >= u_rate {
                    // Try claiming
                    if db::claim_game(&conn, &username, &game.id).unwrap_or(false) {
                        let _ = db::add_spins(&conn, &username, 1);
                        games_claimed += 1;
                    }
                }
            }
        }
    }

    // 3. Fetch and evaluate puzzles
    // A spin for every 25 puzzles solved correctly with puzzle rating >= user's rating at time of play
    let token_str = token.unwrap_or_else(|| "mock_token".to_string());
    let puzzles = match lichess::fetch_puzzle_activity(&token_str, 50).await {
        Ok(p) => p,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": format!("Puzzles fetch failed: {}", e) }))).into_response(),
    };

    // Reconstruct rating progression and process claims inside a block to drop the lock before returning
    let (spins_earned_from_puzzles, total_eligible) = {
        let conn = state.db.lock().unwrap();
        // Sort chronologically (oldest first) to track rating progress correctly
        let mut puzzles_chronological = puzzles.clone();
        puzzles_chronological.sort_by_key(|p| p.date);

        // Reconstruct rating progression walking forward
        // Since we know the ending puzzle_rating, let's work out the starting rating
        let mut wins = 0;
        let mut losses = 0;
        for p in &puzzles_chronological {
            if p.win {
                wins += 1;
            } else {
                losses += 1;
            }
        }

        let mut sim_rating = std::cmp::max(600, puzzle_rating - (wins * 10 - losses * 10));
        let mut eligible_puzzle_ids = Vec::new();

        for p in puzzles_chronological {
            let before_play_rating = sim_rating;
            
            // Correct and rating >= rating before play
            if p.win && p.puzzle.rating >= before_play_rating {
                // Check if already claimed
                let claimed = conn.query_row(
                    "SELECT COUNT(*) FROM claimed_puzzles WHERE username = ?1 AND puzzle_id = ?2",
                    params![username, p.puzzle.id],
                    |row| row.get::<_, i64>(0)
                ).unwrap_or(0) > 0;
                
                if !claimed {
                    eligible_puzzle_ids.push(p.puzzle.id.clone());
                }
            }

            // Adjust rating based on result
            if p.win {
                sim_rating += 10;
            } else {
                sim_rating -= 10;
            }
        }

        // Award 1 spin for every group of 25 eligible puzzles
        let total = eligible_puzzle_ids.len();
        let num_spins = total / 25;
        let mut spins = 0;

        for i in 0..num_spins {
            // Claim 25 puzzles
            for j in 0..25 {
                let p_id = &eligible_puzzle_ids[i * 25 + j];
                let _ = db::claim_puzzle(&conn, &username, p_id);
            }
            let _ = db::add_spins(&conn, &username, 1);
            spins += 1;
        }
        (spins, total)
    };

    let updated_user = {
        let conn = state.db.lock().unwrap();
        db::get_user(&conn, &username).unwrap().unwrap()
    };

    Json(serde_json::json!({
        "success": true,
        "games_sync_spins": games_claimed,
        "puzzles_sync_spins": spins_earned_from_puzzles,
        "puzzles_processed": puzzles.len(),
        "eligible_unclaimed_puzzles": total_eligible % 25, // remainder towards next spin
        "spins_available": updated_user.spins_available,
        "coins": updated_user.coins
    })).into_response()
}

// -------------------------------------------------------------
// SPIN THE WHEEL
// -------------------------------------------------------------

#[derive(Serialize)]
pub struct SpinResponse {
    pub success: bool,
    pub piece: String,     // "pawn", "knight", "bishop", "rook", "queen"
    pub coins_won: i32,    // 1, 3, 3, 5, 9
    pub current_spins: i32,
    pub current_coins: i32,
}

pub async fn spin_wheel(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let username = match get_username(&headers) {
        Some(name) => name,
        None => return Err((StatusCode::UNAUTHORIZED, Json(serde_json::json!({ "error": "Not logged in" })))),
    };

    let conn = state.db.lock().unwrap();

    // Attempt to use a spin
    match db::use_spin(&conn, &username) {
        Ok(true) => {
            // Spin was successful, select a piece with weights:
            // Pawn (1 coin) - 40%
            // Knight (3 coins) - 25%
            // Bishop (3 coins) - 20%
            // Rook (5 coins) - 10%
            // Queen (9 coins) - 5%
            let mut rng = rand::thread_rng();
            let roll = rng.gen_range(0..100);

            let (piece, coins_won) = if roll < 40 {
                ("pawn", 1)
            } else if roll < 65 {
                ("knight", 3)
            } else if roll < 85 {
                ("bishop", 3)
            } else if roll < 95 {
                ("rook", 5)
            } else {
                ("queen", 9)
            };

            // Reward user
            let new_coins = db::reward_coins(&conn, &username, coins_won).unwrap();
            let user = db::get_user(&conn, &username).unwrap().unwrap();

            Ok(Json(SpinResponse {
                success: true,
                piece: piece.to_string(),
                coins_won,
                current_spins: user.spins_available,
                current_coins: new_coins,
            }))
        }
        Ok(false) => {
            Err((
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "error": "No spins available! Sync your Lichess games or puzzles to earn spins." })),
            ))
        }
        Err(e) => {
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            ))
        }
    }
}

// -------------------------------------------------------------
// SHOP & AVATAR EDITOR
// -------------------------------------------------------------

#[derive(Serialize)]
pub struct ShopResponseItem {
    pub id: String,
    pub name: String,
    pub category: String,
    pub price: i32,
    pub asset_url: String,
    pub owned: bool,
}

pub async fn get_shop(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let username = match get_username(&headers) {
        Some(name) => name,
        None => return Err((StatusCode::UNAUTHORIZED, Json(serde_json::json!({ "error": "Not logged in" })))),
    };

    let conn = state.db.lock().unwrap();
    let owned_items = db::get_inventory(&conn, &username)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": e.to_string() }))))?;

    let shop_items = routes_shop_items();
    let response: Vec<ShopResponseItem> = shop_items
        .into_iter()
        .map(|item| {
            let owned = owned_items.contains(&item.id.to_string());
            ShopResponseItem {
                id: item.id.to_string(),
                name: item.name.to_string(),
                category: item.category.to_string(),
                price: item.price,
                asset_url: item.asset_url.to_string(),
                owned,
            }
        })
        .collect();

    Ok(Json(response))
}

#[derive(Deserialize)]
pub struct BuyItemRequest {
    pub item_id: String,
}

pub async fn buy_item(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<BuyItemRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let username = match get_username(&headers) {
        Some(name) => name,
        None => return Err((StatusCode::UNAUTHORIZED, Json(serde_json::json!({ "error": "Not logged in" })))),
    };

    // Find the item in our shop catalog
    let shop_items = routes_shop_items();
    let item = match shop_items.iter().find(|i| i.id == payload.item_id) {
        Some(i) => i,
        None => return Err((StatusCode::BAD_REQUEST, Json(serde_json::json!({ "error": "Item not found in catalog" })))),
    };

    let conn = state.db.lock().unwrap();
    match db::buy_item(&conn, &username, &item.id, item.price) {
        Ok(Ok(new_coins)) => Ok(Json(serde_json::json!({ "success": true, "coins": new_coins }))),
        Ok(Err(e)) => Err((StatusCode::BAD_REQUEST, Json(serde_json::json!({ "error": e })))),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": e.to_string() })))),
    }
}

#[derive(Deserialize)]
pub struct EquipItemRequest {
    pub category: String,
    pub item_id: Option<String>, // None means unequip
}

pub async fn equip_item(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<EquipItemRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let username = match get_username(&headers) {
        Some(name) => name,
        None => return Err((StatusCode::UNAUTHORIZED, Json(serde_json::json!({ "error": "Not logged in" })))),
    };

    let conn = state.db.lock().unwrap();
    match db::equip_item(&conn, &username, &payload.category, payload.item_id.as_deref()) {
        Ok(Ok(())) => {
            let equipped = db::get_equipped(&conn, &username).unwrap_or_default();
            Ok(Json(serde_json::json!({ "success": true, "equipped": equipped })))
        }
        Ok(Err(e)) => Err((StatusCode::BAD_REQUEST, Json(serde_json::json!({ "error": e })))),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": e.to_string() })))),
    }
}

// -------------------------------------------------------------
// FRIENDS PAGE
// -------------------------------------------------------------

#[derive(Serialize)]
pub struct FriendProfile {
    pub username: String,
    pub avatar_base: String,
    pub current_game_rating: i32,
    pub current_puzzle_rating: i32,
    pub equipped: EquippedItems,
    pub lichess_url: String,
}

async fn try_discover_remote_profile(f_name: &str) -> Option<FriendProfile> {
    // 1. Fetch public profile from Lichess
    let pub_prof = lichess::fetch_public_profile(f_name).await.ok()?;
    let links = pub_prof.profile.as_ref()?.links.as_ref()?;

    // 2. Parse links to find LichessKids profile URL candidate
    let mut remote_url = None;
    for line in links.lines() {
        let url_str = line.trim();
        if url_str.starts_with("http") && url_str.contains(&format!("/user/{}", f_name)) {
            if let Some(domain) = url_str.split('/').nth(2) {
                let protocol = if url_str.starts_with("https") { "https" } else { "http" };
                let nodeinfo_url = format!("{}://{}/.well-known/nodeinfo", protocol, domain);

                let client = reqwest::Client::builder()
                    .timeout(std::time::Duration::from_secs(2))
                    .build()
                    .ok()?;

                if let Ok(res) = client.get(&nodeinfo_url).send().await {
                    if res.status().is_success() {
                        #[derive(Deserialize)]
                        struct NodeInfoLink {
                            rel: String,
                            href: String,
                        }
                        #[derive(Deserialize)]
                        struct NodeInfoMeta {
                            links: Vec<NodeInfoLink>,
                        }
                        if let Ok(meta) = res.json::<NodeInfoMeta>().await {
                            for link in meta.links {
                                if link.rel.contains("nodeinfo") {
                                    let href = if link.href.starts_with("http") {
                                        link.href.clone()
                                    } else {
                                        format!("{}://{}{}", protocol, domain, link.href)
                                    };
                                    #[derive(Deserialize)]
                                    struct Software {
                                        name: String,
                                    }
                                    #[derive(Deserialize)]
                                    struct NodeInfo2 {
                                        software: Software,
                                    }
                                    if let Ok(res2) = client.get(&href).send().await {
                                        if let Ok(node2) = res2.json::<NodeInfo2>().await {
                                            if node2.software.name == "lichesskids" {
                                                remote_url = Some(url_str.to_string());
                                                break;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        if remote_url.is_some() {
            break;
        }
    }

    let url = remote_url?;

    // 3. Scrape remote profile page
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(2))
        .build()
        .ok()?;

    let res = client.get(&url).header("User-Agent", "LichessKids-App/1.0").send().await.ok()?;
    let html = res.text().await.ok()?;

    let start_tag = r#"<script type="application/ld+json">"#;
    let end_tag = "</script>";
    let start_idx = html.find(start_tag)?;
    let content = &html[start_idx + start_tag.len()..];
    let end_idx = content.find(end_tag)?;
    let json_str = &content[..end_idx].trim();

    #[derive(Deserialize)]
    struct PersonSchema {
        description: Option<String>,
    }
    #[derive(Deserialize)]
    struct ProfilePageSchema {
        #[serde(rename = "mainEntity")]
        main_entity: PersonSchema,
    }

    let schema: ProfilePageSchema = serde_json::from_str(json_str).ok()?;
    let desc = schema.main_entity.description?;

    let mut avatar_base = "kid_boy".to_string();
    let mut equipped = EquippedItems::default();

    for part in desc.split(';') {
        let kv: Vec<&str> = part.split(':').collect();
        if kv.len() == 2 {
            let key = kv[0].trim();
            let val = kv[1].trim().to_string();
            if !val.is_empty() {
                match key {
                    "avatar_base" => avatar_base = val,
                    "top" => equipped.top = Some(val),
                    "bottom" => equipped.bottom = Some(val),
                    "hat" => equipped.hat = Some(val),
                    "hair" => equipped.hair = Some(val),
                    "accessory" => equipped.accessory = Some(val),
                    "background" => equipped.background = Some(val),
                    _ => {}
                }
            }
        }
    }

    let g_rate = pub_prof.perfs.blitz.or(pub_prof.perfs.rapid).map(|x| x.rating).unwrap_or(1500);
    let p_rate = pub_prof.perfs.puzzle.map(|x| x.rating).unwrap_or(1500);

    Some(FriendProfile {
        username: f_name.to_string(),
        avatar_base,
        current_game_rating: g_rate,
        current_puzzle_rating: p_rate,
        equipped,
        lichess_url: format!("https://lichess.org/@/{}", f_name),
    })
}

pub async fn get_friends(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let username = match get_username(&headers) {
        Some(name) => name,
        None => return Err((StatusCode::UNAUTHORIZED, Json(serde_json::json!({ "error": "Not logged in" })))),
    };

    let token = {
        let conn = state.db.lock().unwrap();
        conn.query_row(
            "SELECT access_token FROM lichess_tokens WHERE username = ?1",
            params![username],
            |row| row.get::<_, String>(0),
        )
        .ok()
    };

    let followed = match lichess::fetch_following(&token.unwrap_or_else(|| "mock_token".to_string())).await {
        Ok(lst) => lst,
        Err(e) => return Err((StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": format!("Following fetch failed: {}", e) })))),
    };

    let mut friends_profiles = Vec::new();
    let mut futures = Vec::new();

    for f_name in followed {
        // Check local DB
        let local_user = {
            let conn = state.db.lock().unwrap();
            db::get_user(&conn, &f_name).ok().flatten()
        };

        if let Some(user) = local_user {
            let equipped = {
                let conn = state.db.lock().unwrap();
                db::get_equipped(&conn, &f_name).unwrap_or_default()
            };
            friends_profiles.push(FriendProfile {
                username: f_name.clone(),
                avatar_base: user.avatar_base,
                current_game_rating: user.current_game_rating,
                current_puzzle_rating: user.current_puzzle_rating,
                equipped,
                lichess_url: format!("https://lichess.org/@/{}", f_name),
            });
        } else {
            // Check remote profile page discovery
            let f_name_clone = f_name.clone();
            futures.push(tokio::spawn(async move {
                // If it fails or is remote, try to discover it
                try_discover_remote_profile(&f_name_clone).await
            }));
        }
    }

    for handle in futures {
        if let Ok(Some(prof)) = handle.await {
            friends_profiles.push(prof);
        }
    }

    Ok(Json(friends_profiles))
}

#[derive(Deserialize)]
pub struct AddFriendRequest {
    pub friend_username: String,
}

pub async fn add_friend(
    Json(_payload): Json<AddFriendRequest>,
) -> Response {
    (StatusCode::BAD_REQUEST, Json(serde_json::json!({ "error": "Follow players directly on Lichess to sync them to Lichess Kids!" }))).into_response()
}

#[derive(Deserialize)]
pub struct DeleteFriendRequest {
    pub friend_username: String,
}

pub async fn delete_friend(
    Json(_payload): Json<DeleteFriendRequest>,
) -> Response {
    (StatusCode::BAD_REQUEST, Json(serde_json::json!({ "error": "Unfollow players directly on Lichess to remove them from Lichess Kids!" }))).into_response()
}

// -------------------------------------------------------------
// STATIC SHOP CATALOG DEFINITION
// -------------------------------------------------------------

pub struct ShopItemCatalog {
    pub id: &'static str,
    pub name: &'static str,
    pub category: &'static str,
    pub price: i32,
    pub asset_url: &'static str,
}

fn routes_shop_items() -> Vec<ShopItemCatalog> {
    vec![
        // Tops
        ShopItemCatalog { id: "superhero_cape", name: "Superhero Cape", category: "top", price: 10, asset_url: "/static/assets/items/superhero_cape.png" },
        ShopItemCatalog { id: "hoodie", name: "Cool Hoodie", category: "top", price: 15, asset_url: "/static/assets/items/hoodie.png" },
        ShopItemCatalog { id: "royal_robe", name: "Royal Robe", category: "top", price: 25, asset_url: "/static/assets/items/royal_robe.png" },
        // Bottoms
        ShopItemCatalog { id: "denim_shorts", name: "Denim Shorts", category: "bottom", price: 5, asset_url: "/static/assets/items/denim_shorts.png" },
        ShopItemCatalog { id: "grass_skirt", name: "Grass Skirt", category: "bottom", price: 10, asset_url: "/static/assets/items/grass_skirt.png" },
        // Hats
        ShopItemCatalog { id: "party_hat", name: "Party Hat", category: "hat", price: 5, asset_url: "/static/assets/items/party_hat.png" },
        ShopItemCatalog { id: "crown", name: "Golden Crown", category: "hat", price: 30, asset_url: "/static/assets/items/crown.png" },
        ShopItemCatalog { id: "cowboy_hat", name: "Cowboy Hat", category: "hat", price: 20, asset_url: "/static/assets/items/cowboy_hat.png" },
        ShopItemCatalog { id: "pirate_hat", name: "Pirate Hat", category: "hat", price: 18, asset_url: "/static/assets/items/pirate_hat.png" },
        // Hair
        ShopItemCatalog { id: "mohawk", name: "Cool Mohawk", category: "hair", price: 12, asset_url: "/static/assets/items/mohawk.png" },
        ShopItemCatalog { id: "rainbow_hair", name: "Rainbow Hair", category: "hair", price: 15, asset_url: "/static/assets/items/rainbow_hair.png" },
        // Accessories
        ShopItemCatalog { id: "magic_wand", name: "Magic Wand", category: "accessory", price: 25, asset_url: "/static/assets/items/magic_wand.png" },
        ShopItemCatalog { id: "sword", name: "Toy Sword", category: "accessory", price: 20, asset_url: "/static/assets/items/sword.png" },
        ShopItemCatalog { id: "balloon", name: "Red Balloon", category: "accessory", price: 8, asset_url: "/static/assets/items/balloon.png" },
        // Backgrounds
        ShopItemCatalog { id: "space", name: "Space Galaxy", category: "background", price: 40, asset_url: "/static/assets/backgrounds/space.png" },
        ShopItemCatalog { id: "forest", name: "Magical Forest", category: "background", price: 30, asset_url: "/static/assets/backgrounds/forest.png" },
        ShopItemCatalog { id: "castle", name: "Candy Castle", category: "background", price: 35, asset_url: "/static/assets/backgrounds/castle.png" },
    ]
}

// -------------------------------------------------------------
// FEDERATION / NODEINFO / PROFILE SCRAPING
// -------------------------------------------------------------

pub async fn get_well_known_nodeinfo() -> impl IntoResponse {
    let json = serde_json::json!({
        "links": [
            {
                "rel": "http://nodeinfo.diaspora.gene.ar/schema/2.0",
                "href": "/nodeinfo/2.0"
            }
        ]
    });
    (StatusCode::OK, Json(json))
}

pub async fn get_nodeinfo_2_0(
    State(state): State<AppState>,
) -> impl IntoResponse {
    let user_count = {
        let conn = state.db.lock().unwrap();
        conn.query_row("SELECT COUNT(*) FROM users", params![], |row| row.get::<_, i64>(0)).unwrap_or(0)
    };

    let json = serde_json::json!({
        "version": "2.0",
        "software": {
            "name": "lichesskids",
            "version": "0.1.0"
        },
        "protocols": [
            "http"
        ],
        "services": {
            "inbound": [],
            "outbound": []
        },
        "openRegistrations": true,
        "usage": {
            "users": {
                "total": user_count
            }
        },
        "metadata": {}
    });
    (StatusCode::OK, Json(json))
}

pub async fn get_user_profile_html(
    Path(username): Path<String>,
    State(state): State<AppState>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let conn = state.db.lock().unwrap();
    let user_profile = match db::get_user(&conn, &username) {
        Ok(Some(u)) => u,
        _ => {
            return (StatusCode::NOT_FOUND, Html("<h1>User Not Found</h1>")).into_response();
        }
    };

    let equipped = db::get_equipped(&conn, &username).unwrap_or_default();

    let mut query_params = format!("base={}", user_profile.avatar_base);
    if let Some(ref t) = equipped.top { query_params.push_str(&format!("&top={}", t)); }
    if let Some(ref b) = equipped.bottom { query_params.push_str(&format!("&bottom={}", b)); }
    if let Some(ref h) = equipped.hat { query_params.push_str(&format!("&hat={}", h)); }
    if let Some(ref hr) = equipped.hair { query_params.push_str(&format!("&hair={}", hr)); }
    if let Some(ref a) = equipped.accessory { query_params.push_str(&format!("&accessory={}", a)); }
    if let Some(ref bg) = equipped.background { query_params.push_str(&format!("&background={}", bg)); }

    let host = headers.get("host")
        .and_then(|h| h.to_str().ok())
        .map(|h| format!("http://{}", h))
        .unwrap_or_else(|| "http://localhost:3000".to_string());

    let avatar_svg_url = format!("{}/api/avatar-svg/{}?{}", host, username, query_params);

    let html = format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>{username}'s Lichess Kids Profile</title>
    <link rel="stylesheet" href="/static/style.css">
    <script type="application/ld+json">
    {{
      "@context": "https://schema.org",
      "@type": "ProfilePage",
      "mainEntity": {{
        "@type": "Person",
        "name": "{username}",
        "image": "{avatar_svg_url}",
        "description": "avatar_base:{avatar_base};top:{top};bottom:{bottom};hat:{hat};hair:{hair};accessory:{accessory};background:{background}"
      }}
    }}
    </script>
</head>
<body style="background-color: var(--bg-primary); display: flex; align-items: center; justify-content: center; height: 100vh;">
    <div class="glass-panel" style="max-width: 400px; width: 90%; text-align: center; display: flex; flex-direction: column; align-items: center; gap: 20px;">
        <div id="avatar-container" class="avatar-preview" style="width: 200px; height: 200px;"></div>
        <h2>{username}</h2>
        <p style="color: var(--text-secondary);">Lichess Kids Profile</p>
        <p style="font-size: 0.95rem; color: var(--text-secondary);">
            Game Rating: <span style="font-weight:700;color:var(--knight-color);">{game_rating}</span> |
            Puzzles: <span style="font-weight:700;color:var(--bishop-color);">{puzzle_rating}</span>
        </p>
        <a href="https://lichess.org/@/{username}" target="_blank" class="btn btn-secondary" style="width: 100%;">View on Lichess</a>
    </div>

    <script type="module">
        import {{ renderAvatar }} from '/static/avatar-renderer.js';
        const equipped = {{
            top: {top_json},
            bottom: {bottom_json},
            hat: {hat_json},
            hair: {hair_json},
            accessory: {accessory_json},
            background: {background_json}
        }};
        renderAvatar(document.getElementById('avatar-container'), "{avatar_base}", equipped);
    </script>
</body>
</html>"#,
        username = username,
        avatar_base = user_profile.avatar_base,
        avatar_svg_url = avatar_svg_url,
        game_rating = user_profile.current_game_rating,
        puzzle_rating = user_profile.current_puzzle_rating,
        top = equipped.top.as_deref().unwrap_or(""),
        bottom = equipped.bottom.as_deref().unwrap_or(""),
        hat = equipped.hat.as_deref().unwrap_or(""),
        hair = equipped.hair.as_deref().unwrap_or(""),
        accessory = equipped.accessory.as_deref().unwrap_or(""),
        background = equipped.background.as_deref().unwrap_or(""),
        top_json = serde_json::to_string(&equipped.top).unwrap(),
        bottom_json = serde_json::to_string(&equipped.bottom).unwrap(),
        hat_json = serde_json::to_string(&equipped.hat).unwrap(),
        hair_json = serde_json::to_string(&equipped.hair).unwrap(),
        accessory_json = serde_json::to_string(&equipped.accessory).unwrap(),
        background_json = serde_json::to_string(&equipped.background).unwrap()
    );

    (StatusCode::OK, Html(html)).into_response()
}

#[derive(Deserialize, Debug)]
pub struct AvatarSvgQuery {
    pub base: Option<String>,
    pub top: Option<String>,
    pub bottom: Option<String>,
    pub hat: Option<String>,
    pub hair: Option<String>,
    pub accessory: Option<String>,
    pub background: Option<String>,
}

pub async fn get_avatar_svg(
    Path(username): Path<String>,
    State(state): State<AppState>,
    Query(query): Query<AvatarSvgQuery>,
) -> impl IntoResponse {
    let (base, equipped) = {
        if let Some(ref base_param) = query.base {
            (
                base_param.clone(),
                EquippedItems {
                    top: query.top.clone(),
                    bottom: query.bottom.clone(),
                    hat: query.hat.clone(),
                    hair: query.hair.clone(),
                    accessory: query.accessory.clone(),
                    background: query.background.clone(),
                }
            )
        } else {
            let conn = state.db.lock().unwrap();
            let base = match db::get_user(&conn, &username) {
                Ok(Some(u)) => u.avatar_base,
                _ => "kid_boy".to_string(),
            };
            let equipped = db::get_equipped(&conn, &username).unwrap_or_default();
            (base, equipped)
        }
    };

    let svg = build_avatar_svg_string(&base, &equipped);

    let mut headers = HeaderMap::new();
    headers.insert("Content-Type", "image/svg+xml".parse().unwrap());
    headers.insert("Cache-Control", "no-cache, no-store, must-revalidate".parse().unwrap());
    (StatusCode::OK, headers, svg)
}

fn get_base_svg_paths(base: &str) -> &'static str {
    match base {
        "kid_boy" => r##"
            <!-- Body -->
            <path d="M 60 180 C 60 150, 140 150, 140 180 Z" fill="#4dabf7" stroke="#1c7ed6" stroke-width="3" />
            <path d="M 90 155 L 90 170 L 110 170 L 110 155 Z" fill="#ffd8a8" stroke="#f76707" stroke-width="2" />
            <!-- Head -->
            <circle cx="100" cy="100" r="50" fill="#ffe066" stroke="#f59f00" stroke-width="3" />
            <!-- Eyes -->
            <circle cx="85" cy="95" r="7" fill="#212529" />
            <circle cx="83" cy="93" r="2.5" fill="#fff" />
            <circle cx="115" cy="95" r="7" fill="#212529" />
            <circle cx="113" cy="93" r="2.5" fill="#fff" />
            <!-- Smile -->
            <path d="M 85 118 Q 100 135 115 118" fill="none" stroke="#e03131" stroke-width="4" stroke-linecap="round" />
            <!-- Cheeks -->
            <circle cx="75" cy="110" r="6" fill="#ffc9c9" opacity="0.6" />
            <circle cx="125" cy="110" r="6" fill="#ffc9c9" opacity="0.6" />
            <!-- Default Hair -->
            <path d="M 50 100 C 45 60, 155 60, 150 100 C 130 90, 70 90, 50 100 Z" fill="#862e9c" />
        "##,
        "kid_girl" => r##"
            <!-- Body -->
            <path d="M 60 180 C 60 150, 140 150, 140 180 Z" fill="#ff8787" stroke="#fa5252" stroke-width="3" />
            <path d="M 90 155 L 90 170 L 110 170 L 110 155 Z" fill="#ffd8a8" stroke="#f76707" stroke-width="2" />
            <!-- Head -->
            <circle cx="100" cy="100" r="50" fill="#ffd8a8" stroke="#e67e22" stroke-width="3" />
            <!-- Eyes -->
            <circle cx="85" cy="95" r="7" fill="#212529" />
            <circle cx="83" cy="93" r="2.5" fill="#fff" />
            <circle cx="115" cy="95" r="7" fill="#212529" />
            <circle cx="113" cy="93" r="2.5" fill="#fff" />
            <!-- Smile -->
            <path d="M 85 118 Q 100 135 115 118" fill="none" stroke="#e03131" stroke-width="4" stroke-linecap="round" />
            <!-- Hair Braids -->
            <circle cx="45" cy="115" r="14" fill="#d9480f" />
            <circle cx="155" cy="115" r="14" fill="#d9480f" />
            <path d="M 50 100 C 45 55, 155 55, 150 100 Z" fill="#d9480f" />
        "##,
        "cat" => r##"
            <!-- Body -->
            <path d="M 60 180 C 60 145, 140 145, 140 180 Z" fill="#ff922b" stroke="#d9480f" stroke-width="3" />
            <!-- Ears -->
            <polygon points="50,70 80,40 85,85" fill="#ff922b" stroke="#d9480f" stroke-width="3" />
            <polygon points="58,68 78,48 81,78" fill="#ffc078" />
            <polygon points="150,70 120,40 115,85" fill="#ff922b" stroke="#d9480f" stroke-width="3" />
            <polygon points="142,68 122,48 119,78" fill="#ffc078" />
            <!-- Head -->
            <circle cx="100" cy="100" r="50" fill="#ff922b" stroke="#d9480f" stroke-width="3" />
            <!-- Eyes -->
            <ellipse cx="80" cy="95" rx="6" ry="8" fill="#212529" />
            <circle cx="78" cy="92" r="2.5" fill="#fff" />
            <ellipse cx="120" cy="95" rx="6" ry="8" fill="#212529" />
            <circle cx="118" cy="92" r="2.5" fill="#fff" />
            <!-- Snout & Nose -->
            <polygon points="96,108 104,108 100,113" fill="#e03131" />
            <path d="M 94 116 Q 100 122 106 116" fill="none" stroke="#212529" stroke-width="2" />
            <!-- Whiskers -->
            <line x1="72" y1="112" x2="45" y2="108" stroke="#495057" stroke-width="2" />
            <line x1="72" y1="117" x2="42" y2="119" stroke="#495057" stroke-width="2" />
            <line x1="128" y1="112" x2="155" y2="108" stroke="#495057" stroke-width="2" />
            <line x1="128" y1="117" x2="158" y2="119" stroke="#495057" stroke-width="2" />
        "##,
        "dog" => r##"
            <!-- Body -->
            <path d="M 60 180 C 60 145, 140 145, 140 180 Z" fill="#a9e34b" stroke="#74b816" stroke-width="3" />
            <!-- Floppy Ears -->
            <path d="M 45 75 C 30 75, 35 125, 55 120 Z" fill="#868e96" stroke="#495057" stroke-width="3" />
            <path d="M 155 75 C 170 75, 165 125, 145 120 Z" fill="#868e96" stroke="#495057" stroke-width="3" />
            <!-- Head -->
            <circle cx="100" cy="100" r="50" fill="#f1f3f5" stroke="#ced4da" stroke-width="3" />
            <!-- Spots -->
            <ellipse cx="80" cy="90" rx="14" ry="18" fill="#868e96" opacity="0.4" />
            <!-- Eyes -->
            <circle cx="80" cy="95" r="7" fill="#212529" />
            <circle cx="78" cy="92" r="2.5" fill="#fff" />
            <circle cx="120" cy="95" r="7" fill="#212529" />
            <circle cx="118" cy="92" r="2.5" fill="#fff" />
            <!-- Nose & Mouth -->
            <ellipse cx="100" cy="112" rx="7" ry="5" fill="#212529" />
            <path d="M 94 118 Q 100 125 106 118" fill="none" stroke="#212529" stroke-width="2.5" stroke-linecap="round" />
        "##,
        "alien" => r##"
            <!-- Body -->
            <path d="M 60 180 C 60 145, 140 145, 140 180 Z" fill="#69db7c" stroke="#2b8a3e" stroke-width="3" />
            <!-- Antennae -->
            <line x1="80" y1="58" x2="65" y2="35" stroke="#2b8a3e" stroke-width="4" />
            <circle cx="65" cy="35" r="8" fill="#ffd23f" stroke="#2b8a3e" stroke-width="2" />
            <line x1="120" y1="58" x2="135" y2="35" stroke="#2b8a3e" stroke-width="4" />
            <circle cx="135" cy="35" r="8" fill="#ffd23f" stroke="#2b8a3e" stroke-width="2" />
            <!-- Head -->
            <ellipse cx="100" cy="105" rx="55" ry="45" fill="#69db7c" stroke="#2b8a3e" stroke-width="3" />
            <!-- Three Eyes -->
            <circle cx="75" cy="98" r="8" fill="#fff" stroke="#2b8a3e" stroke-width="1.5" />
            <circle cx="75" cy="98" r="4" fill="#364fc7" />
            <circle cx="100" cy="90" r="10" fill="#fff" stroke="#2b8a3e" stroke-width="1.5" />
            <circle cx="100" cy="90" r="5" fill="#364fc7" />
            <circle cx="125" cy="98" r="8" fill="#fff" stroke="#2b8a3e" stroke-width="1.5" />
            <circle cx="125" cy="98" r="4" fill="#364fc7" />
            <!-- Mouth -->
            <path d="M 85 125 Q 100 138 115 125" fill="none" stroke="#2b8a3e" stroke-width="4" stroke-linecap="round" />
        "##,
        _ => "",
    }
}

fn get_item_svg_paths(item: &str) -> &'static str {
    match item {
        "party_hat" => r##"
            <polygon points="100,10 70,60 130,60" fill="#ff6b6b" stroke="#e03131" stroke-width="2" />
            <circle cx="100" cy="10" r="6" fill="#ffd23f" />
            <line x1="80" y1="40" x2="120" y2="40" stroke="#fff" stroke-dasharray="3,3" stroke-width="2" />
        "##,
        "crown" => r##"
            <polygon points="65,65 75,40 100,55 125,40 135,65" fill="#ffd23f" stroke="#f59f00" stroke-width="2" />
            <rect x="65" y="65" width="70" height="8" fill="#ffd23f" stroke="#f59f00" stroke-width="2" />
            <circle cx="75" cy="40" r="3" fill="#ff1744" />
            <circle cx="100" cy="55" r="3" fill="#00e676" />
            <circle cx="125" cy="40" r="3" fill="#364fc7" />
        "##,
        "cowboy_hat" => r##"
            <ellipse cx="100" cy="62" rx="35" ry="18" fill="#85583f" stroke="#5c3826" stroke-width="2" />
            <path d="M 50 68 Q 100 78 150 68 C 145 60, 55 60, 50 68 Z" fill="#a06a42" stroke="#5c3826" stroke-width="2" />
        "##,
        "pirate_hat" => r##"
            <path d="M 55 68 Q 100 48 145 68 C 135 60, 65 60, 55 68 Z" fill="#212529" stroke="#000" stroke-width="2" />
            <path d="M 75 62 Q 100 66 125 62 Q 100 78 75 62 Z" fill="#212529" stroke="#000" stroke-width="2" />
            <circle cx="100" cy="62" r="3.5" fill="#fff" />
            <line x1="95" y1="58" x2="105" y2="66" stroke="#fff" stroke-width="1.2" />
            <line x1="105" y1="58" x2="95" y2="66" stroke="#fff" stroke-width="1.2" />
        "##,
        "superhero_cape" => r##"
            <path d="M 68 140 L 40 195 L 160 195 L 132 140 Z" fill="#ff1744" stroke="#d50000" stroke-width="2" />
        "##,
        "hoodie" => r##"
            <path d="M 60 180 C 60 148, 140 148, 140 180 Z" fill="#ff6b6b" stroke="#e03131" stroke-width="2.5" />
            <line x1="94" y1="150" x2="94" y2="168" stroke="#fff" stroke-width="2" stroke-linecap="round" />
            <line x1="106" y1="150" x2="106" y2="168" stroke="#fff" stroke-width="2" stroke-linecap="round" />
        "##,
        "royal_robe" => r##"
            <path d="M 58 180 C 58 146, 142 146, 142 180 Z" fill="#862e9c" stroke="#6b21a8" stroke-width="2.5" />
            <path d="M 85 148 L 100 180 L 115 148 Z" fill="#ffd23f" />
            <ellipse cx="100" cy="146" rx="20" ry="6" fill="#f8f9fa" />
        "##,
        "denim_shorts" => r##"
            <path d="M 64 175 C 64 168, 136 168, 136 175 L 134 188 L 102 188 L 102 180 L 98 180 L 98 188 L 66 188 Z" fill="#364fc7" stroke="#1b2e88" stroke-width="2" />
        "##,
        "grass_skirt" => r##"
            <rect x="62" y="166" width="76" height="4" fill="#a06a42" />
            <path d="M 64 170 L 68 190 M 74 170 L 78 192 M 84 170 L 87 194 M 94 170 L 96 195 M 104 170 L 103 195 M 114 170 L 111 194 M 124 170 L 120 192 M 134 170 L 130 190" stroke="#51cf66" stroke-width="5" stroke-linecap="round" />
        "##,
        "mohawk" => r##"
            <path d="M 100,25 C 103,45, 97,45, 100,55" fill="none" stroke="#f06595" stroke-width="12" stroke-linecap="round" />
            <path d="M 100,25 L 94,40 M 100,30 L 106,45 M 100,20 L 92,35" stroke="#f285c7" stroke-width="3" />
        "##,
        "rainbow_hair" => r##"
            <defs>
                <linearGradient id="rainbow-grad" x1="0%" y1="0%" x2="100%" y2="0%">
                    <stop offset="0%" stop-color="#ff1744" />
                    <stop offset="25%" stop-color="#ffd23f" />
                    <stop offset="50%" stop-color="#00e676" />
                    <stop offset="75%" stop-color="#00e5ff" />
                    <stop offset="100%" stop-color="#b967ff" />
                </linearGradient>
            </defs>
            <path d="M 45 92 C 40 70, 160 70, 155 92 C 160 110, 150 145, 145 155 M 45 92 C 40 110, 50 145, 55 155" fill="none" stroke="url(#rainbow-grad)" stroke-width="8" stroke-linecap="round" />
        "##,
        "magic_wand" => r##"
            <line x1="135" y1="170" x2="175" y2="120" stroke="#85583f" stroke-width="4" stroke-linecap="round" />
            <polygon points="175,120 178,112 186,110 180,104 182,96 175,100 168,96 170,104 164,110 172,112" fill="#ffd23f" stroke="#f59f00" stroke-width="1.5" />
            <circle cx="175" cy="120" r="15" fill="rgba(255, 210, 63, 0.3)" opacity="0.6" />
        "##,
        "sword" => r##"
            <line x1="140" y1="160" x2="160" y2="180" stroke="#f59f00" stroke-width="5" />
            <line x1="135" y1="170" x2="155" y2="150" stroke="#e67e22" stroke-width="8" stroke-linecap="round" />
            <polygon points="135,165 178,110 184,116 141,171" fill="#ced4da" stroke="#868e96" stroke-width="1.5" />
            <line x1="138" y1="168" x2="181" y2="113" stroke="#adb5bd" stroke-width="1.5" />
        "##,
        "balloon" => r##"
            <path d="M 145 160 Q 155 170 148 190" fill="none" stroke="#adb5bd" stroke-width="1.5" />
            <ellipse cx="145" cy="130" rx="18" ry="24" fill="#ff1744" stroke="#d50000" stroke-width="2" />
            <polygon points="142,154 148,154 145,159" fill="#ff1744" stroke="#d50000" stroke-width="1.5" />
            <ellipse cx="138" cy="122" rx="4" ry="7" fill="#fff" opacity="0.6" transform="rotate(-15, 138, 122)" />
        "##,
        "space" => r##"
            <defs>
                <linearGradient id="space-grad" x1="0%" y1="0%" x2="0%" y2="100%">
                    <stop offset="0%" stop-color="#0b091a" />
                    <stop offset="100%" stop-color="#1b1542" />
                </linearGradient>
            </defs>
            <rect x="0" y="0" width="200" height="200" fill="url(#space-grad)" />
            <circle cx="30" cy="40" r="1.5" fill="#fff" opacity="0.8" />
            <circle cx="170" cy="50" r="1" fill="#fff" opacity="0.6" />
            <circle cx="80" cy="160" r="1.5" fill="#fff" opacity="0.9" />
            <circle cx="160" cy="150" r="2" fill="#ffd23f" opacity="0.7" />
            <circle cx="40" cy="140" r="12" fill="#ff7675" />
            <path d="M 22 145 Q 40 135 58 145" fill="none" stroke="#ffd23f" stroke-width="2" />
        "##,
        "forest" => r##"
            <defs>
                <linearGradient id="forest-grad" x1="0%" y1="0%" x2="0%" y2="100%">
                    <stop offset="0%" stop-color="#2b8a3e" />
                    <stop offset="100%" stop-color="#0b3a1a" />
                </linearGradient>
            </defs>
            <rect x="0" y="0" width="200" height="200" fill="url(#forest-grad)" />
            <polygon points="30,120 15,160 45,160" fill="#082c14" />
            <polygon points="30,100 20,130 40,130" fill="#082c14" />
            <polygon points="170,110 155,150 185,150" fill="#082c14" />
            <polygon points="170,90 160,120 180,120" fill="#082c14" />
        "##,
        "castle" => r##"
            <defs>
                <linearGradient id="castle-grad" x1="0%" y1="0%" x2="0%" y2="100%">
                    <stop offset="0%" stop-color="#f06595" />
                    <stop offset="100%" stop-color="#4c0519" />
                </linearGradient>
            </defs>
            <rect x="0" y="0" width="200" height="200" fill="url(#castle-grad)" />
            <rect x="60" y="120" width="80" height="80" fill="#2d0611" />
            <rect x="40" y="100" width="25" height="100" fill="#1f030a" />
            <polygon points="40,100 52.5,70 65,100" fill="#ff8787" />
            <rect x="135" y="100" width="25" height="100" fill="#1f030a" />
            <polygon points="135,100 147.5,70 160,100" fill="#ff8787" />
            <path d="M 85 200 C 85 160, 115 160, 115 200 Z" fill="#f06595" />
        "##,
        _ => "",
    }
}

fn build_avatar_svg_string(base: &str, equipped: &EquippedItems) -> String {
    let mut inner = String::new();

    if let Some(ref bg) = equipped.background {
        inner.push_str(get_item_svg_paths(bg));
    }
    if let Some(ref top) = equipped.top {
        if top == "superhero_cape" {
            inner.push_str(get_item_svg_paths(top));
        }
    }
    inner.push_str(get_base_svg_paths(base));
    if let Some(ref hair) = equipped.hair {
        inner.push_str(get_item_svg_paths(hair));
    }
    if let Some(ref top) = equipped.top {
        if top != "superhero_cape" {
            inner.push_str(get_item_svg_paths(top));
        }
    }
    if let Some(ref bottom) = equipped.bottom {
        inner.push_str(get_item_svg_paths(bottom));
    }
    if let Some(ref hat) = equipped.hat {
        inner.push_str(get_item_svg_paths(hat));
    }
    if let Some(ref acc) = equipped.accessory {
        inner.push_str(get_item_svg_paths(acc));
    }

    format!(
        r#"<svg viewBox="0 0 200 200" width="100%" height="100%" xmlns="http://www.w3.org/2000/svg">{}</svg>"#,
        inner
    )
}

