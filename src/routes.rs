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
    pub assets: Arc<crate::assets::AssetCatalog>,
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

pub async fn oauth_start(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let host = headers.get("host")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("localhost:64355");
    let proto = headers.get("x-forwarded-proto")
        .and_then(|p| p.to_str().ok())
        .unwrap_or("http");
    let redirect_uri = format!("{}://{}/api/oauth/callback", proto, host);

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
        urlencoding::encode(&redirect_uri),
        state_val,
        code_challenge
    );

    Json(serde_json::json!({ "auth_url": auth_url }))
}

#[axum::debug_handler]
pub async fn oauth_callback(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<OAuthCallbackQuery>,
) -> Response {
    let host = headers.get("host")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("localhost:64355");
    let proto = headers.get("x-forwarded-proto")
        .and_then(|p| p.to_str().ok())
        .unwrap_or("http");
    let redirect_uri = format!("{}://{}/api/oauth/callback", proto, host);
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
        ("redirect_uri", &redirect_uri),
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
                "INSERT OR IGNORE INTO users (username, avatar_base, current_game_rating, current_puzzle_rating) VALUES (?1, 'cat', ?2, ?3)",
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

    if !state.assets.bases_map.contains_key(&payload.avatar_base) {
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

    let (token, mut puzzle_rating, last_game_sync, last_puzzle_sync) = {
        let conn = state.db.lock().unwrap();
        let token: Option<String> = conn
            .query_row(
                "SELECT access_token FROM lichess_tokens WHERE username = ?1",
                params![username],
                |row| row.get(0),
            )
            .ok();

        let mut p_rate = 1500;
        let mut g_sync = 0;
        let mut p_sync = 0;
        if let Some(user) = db::get_user(&conn, &username).unwrap() {
            p_rate = user.current_puzzle_rating;
            g_sync = user.last_game_sync;
            p_sync = user.last_puzzle_sync;
        }
        (token, p_rate, g_sync, p_sync)
    };

    let sync_start_time = std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64;

    let token = match token {
        Some(t) => t,
        None => return (StatusCode::BAD_REQUEST, Json(serde_json::json!({ "error": "Lichess token not found. Please log in again." }))).into_response(),
    };

    // 1. Fetch user's rating profile
    if let Ok(p) = lichess::fetch_profile(&token).await {
        let g_rate = p.perfs.blitz.or(p.perfs.rapid).map(|x| x.rating).unwrap_or(1500);
        let p_rate = p.perfs.puzzle.map(|x| x.rating).unwrap_or(1500);
        let conn = state.db.lock().unwrap();
        let _ = db::update_user_ratings(&conn, &username, g_rate, p_rate);
        puzzle_rating = p_rate;
    }

    // 2. Fetch and evaluate games
    // A spin for every person/bot you beat with rating >= user's rating at time of play
    let games = match lichess::fetch_games(&username, &token, Some(last_game_sync)).await {
        Ok(g) => g,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": format!("Games fetch failed: {}", e) }))).into_response(),
    };

    let mut games_eligible = 0;
    let mut games_claimed = 0;
    {
        let conn = state.db.lock().unwrap();
        for game in &games {
            if !game.rated {
                continue;
            }

            let user_won = match game.winner.as_deref() {
                Some("white") => game.players.white.user.as_ref().map(|u| u.id == username.to_lowercase()).unwrap_or(false),
                Some("black") => game.players.black.user.as_ref().map(|u| u.id == username.to_lowercase()).unwrap_or(false),
                _ => false,
            };

            if !user_won {
                continue;
            }

            let (user_game_rating, opp_game_rating) = if game.players.white.user.as_ref().map(|u| u.id == username.to_lowercase()).unwrap_or(false) {
                (game.players.white.rating, game.players.black.rating)
            } else {
                (game.players.black.rating, game.players.white.rating)
            };

            if let (Some(u_rate), Some(o_rate)) = (user_game_rating, opp_game_rating) {
                let min_required_rating = u_rate + state.assets.spin_rules.game_rating_offset;
                if o_rate >= min_required_rating {
                    games_eligible += 1;
                    if db::claim_game(&conn, &username, &game.id).unwrap_or(false) {
                        let _ = db::add_spins(&conn, &username, 1);
                        games_claimed += 1;
                    }
                }
            }
        }
    }

    let puzzles = match lichess::fetch_puzzle_activity(&token, Some(last_puzzle_sync)).await {
        Ok(p) => p,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": format!("Puzzles fetch failed: {}", e) }))).into_response(),
    };

    let mut new_last_game_sync = last_game_sync;
    for g in &games {
        if g.created_at > new_last_game_sync {
            new_last_game_sync = g.created_at;
        }
    }

    let mut new_last_puzzle_sync = last_puzzle_sync;
    for p in &puzzles {
        if p.date > new_last_puzzle_sync {
            new_last_puzzle_sync = p.date;
        }
    }

    let new_puzzles: Vec<_> = puzzles.into_iter().filter(|p| p.date > last_puzzle_sync).collect();

    let mut puzzles_eligible = 0;
    let (spins_earned_from_puzzles, total_eligible) = {
        let conn = state.db.lock().unwrap();
        let mut puzzles_chronological = new_puzzles.clone();
        puzzles_chronological.sort_by_key(|p| p.date);

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
            let min_required_rating = before_play_rating + state.assets.spin_rules.puzzle_rating_offset;
            
            if p.win && p.puzzle.rating >= min_required_rating {
                puzzles_eligible += 1;
                let claimed = conn.query_row(
                    "SELECT COUNT(*) FROM claimed_puzzles WHERE username = ?1 AND puzzle_id = ?2",
                    params![username, p.puzzle.id],
                    |row| row.get::<_, i64>(0)
                ).unwrap_or(0) > 0;
                
                if !claimed {
                    eligible_puzzle_ids.push(p.puzzle.id.clone());
                }
            }

            if p.win {
                sim_rating += 10;
            } else {
                sim_rating -= 10;
            }
        }

        let divisor = std::cmp::max(1, state.assets.spin_rules.puzzles_per_spin as usize);
        let total = eligible_puzzle_ids.len();
        let num_spins = total / divisor;
        let mut spins = 0;

        for i in 0..num_spins {
            for j in 0..divisor {
                let p_id = &eligible_puzzle_ids[i * divisor + j];
                let _ = db::claim_puzzle(&conn, &username, p_id);
            }
            let _ = db::add_spins(&conn, &username, 1);
            spins += 1;
        }
        (spins, total)
    };

    {
        let conn = state.db.lock().unwrap();
        let _ = db::update_sync_timestamps(&conn, &username, new_last_game_sync, new_last_puzzle_sync);
        let _ = db::update_last_synced_at(&conn, &username, sync_start_time);
    }

    let updated_user = {
        let conn = state.db.lock().unwrap();
        db::get_user(&conn, &username).unwrap().unwrap()
    };

    let divisor = std::cmp::max(1, state.assets.spin_rules.puzzles_per_spin as usize);
    Json(serde_json::json!({
        "success": true,
        "games_sync_spins": games_claimed,
        "puzzles_sync_spins": spins_earned_from_puzzles,
        "daily_spin_claimed": false,
        "puzzles_processed": new_puzzles.len(),
        "eligible_unclaimed_puzzles": total_eligible % divisor,
        "puzzles_per_spin": divisor,
        "spins_available": updated_user.spins_available,
        "coins": updated_user.coins,
        "games_eligible": games_eligible,
        "puzzles_eligible": puzzles_eligible,
        "last_game_sync": new_last_game_sync,
        "last_puzzle_sync": new_last_puzzle_sync,
        "last_synced_at": sync_start_time
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

    let response: Vec<ShopResponseItem> = state.assets.items
        .iter()
        .map(|item| {
            let owned = owned_items.contains(&item.id);
            let asset_url = format!("/static/assets/{}.png", item.id);
            ShopResponseItem {
                id: item.id.clone(),
                name: item.name.clone(),
                category: item.category.clone(),
                price: item.price,
                asset_url,
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

    // Find the item in our dynamic assets catalog
    let item = match state.assets.items.iter().find(|i| i.id == payload.item_id) {
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

    let token_str = token.unwrap_or_default();
    let followed = match lichess::fetch_following(&token_str).await {
        Ok(lst) => lst,
        Err(e) => return Err((StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": format!("Following fetch failed: {}", e) })))),
    };

    let mut friends_profiles = Vec::new();

    for f_name in followed {
        // Check local DB case-insensitively to get stored user casing
        let local_user = {
            let conn = state.db.lock().unwrap();
            conn.query_row(
                "SELECT username, avatar_base, current_game_rating, current_puzzle_rating FROM users WHERE LOWER(username) = LOWER(?1)",
                params![f_name],
                |row| Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i32>(2)?,
                    row.get::<_, i32>(3)?
                ))
            ).ok()
        };

        if let Some((stored_username, avatar_base, g_rating, p_rating)) = local_user {
            let equipped = {
                let conn = state.db.lock().unwrap();
                db::get_equipped(&conn, &stored_username).unwrap_or_default()
            };
            friends_profiles.push(FriendProfile {
                username: stored_username.clone(),
                avatar_base,
                current_game_rating: g_rating,
                current_puzzle_rating: p_rating,
                equipped,
                lichess_url: format!("https://lichess.org/@/{}", stored_username),
            });
        }
    }

    Ok(Json(friends_profiles))
}

pub async fn get_assets_catalog(
    State(state): State<AppState>,
) -> impl IntoResponse {
    (StatusCode::OK, Json((*state.assets).clone()))
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
        .unwrap_or("localhost:3000");

    let proto = headers.get("x-forwarded-proto")
        .and_then(|p| p.to_str().ok())
        .unwrap_or("http");

    let avatar_svg_url = format!("{}://{}/api/avatar-svg/{}?{}", proto, host, username, query_params);

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
        import {{ renderAvatar, initAssets }} from '/static/avatar-renderer.js?v=2';
        async function init() {{
            try {{
                const res = await fetch('/api/assets/catalog');
                const catalog = await res.json();
                initAssets(catalog);
            }} catch (err) {{
                console.error("Failed to load catalog:", err);
            }}
            const equipped = {{
                top: {top_json},
                bottom: {bottom_json},
                hat: {hat_json},
                hair: {hair_json},
                accessory: {accessory_json},
                background: {background_json}
            }};
            renderAvatar(document.getElementById('avatar-container'), "{avatar_base}", equipped);
        }}
        init();
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
                _ => "cat".to_string(),
            };
            let equipped = db::get_equipped(&conn, &username).unwrap_or_default();
            (base, equipped)
        }
    };

    let svg = build_avatar_svg_string(&base, &equipped, &state.assets);

    let mut headers = HeaderMap::new();
    headers.insert("Content-Type", "image/svg+xml".parse().unwrap());
    headers.insert("Cache-Control", "no-cache, no-store, must-revalidate".parse().unwrap());
    (StatusCode::OK, headers, svg)
}

fn build_avatar_svg_string(
    base: &str,
    equipped: &EquippedItems,
    assets: &crate::assets::AssetCatalog,
) -> String {
    let mut inner = String::new();

    if let Some(ref bg) = equipped.background {
        if let Some(svg) = assets.items_map.get(bg) {
            inner.push_str(svg);
        }
    }
    if let Some(ref top) = equipped.top {
        if top == "superhero_cape" {
            if let Some(svg) = assets.items_map.get(top) {
                inner.push_str(svg);
            }
        }
    }
    if let Some(svg) = assets.bases_map.get(base) {
        inner.push_str(svg);
    }
    if let Some(ref hair) = equipped.hair {
        if let Some(svg) = assets.items_map.get(hair) {
            inner.push_str(svg);
        }
    }
    if let Some(ref top) = equipped.top {
        if top != "superhero_cape" {
            if let Some(svg) = assets.items_map.get(top) {
                inner.push_str(svg);
            }
        }
    }
    if let Some(ref bottom) = equipped.bottom {
        if let Some(svg) = assets.items_map.get(bottom) {
            inner.push_str(svg);
        }
    }
    if let Some(ref hat) = equipped.hat {
        if let Some(svg) = assets.items_map.get(hat) {
            inner.push_str(svg);
        }
    }
    if let Some(ref acc) = equipped.accessory {
        if let Some(svg) = assets.items_map.get(acc) {
            inner.push_str(svg);
        }
    }

    format!(
        r#"<svg viewBox="0 0 200 200" width="100%" height="100%" xmlns="http://www.w3.org/2000/svg">{}</svg>"#,
        inner
    )
}

pub async fn delete_profile(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let username = match get_username(&headers) {
        Some(name) => name,
        None => return Err((StatusCode::UNAUTHORIZED, Json(serde_json::json!({ "error": "Not logged in" })))),
    };

    let conn = state.db.lock().unwrap();
    match db::delete_user(&conn, &username) {
        Ok(_) => {
            // Delete the username cookie
            let cookie = "username=; Path=/; HttpOnly; Max-Age=0; SameSite=Lax";
            let mut headers = HeaderMap::new();
            headers.insert(
                axum::http::header::SET_COOKIE,
                axum::http::HeaderValue::from_str(cookie).unwrap(),
            );
            Ok((headers, Json(serde_json::json!({ "success": true }))))
        }
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )),
    }
}

