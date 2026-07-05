use axum::{
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Redirect, Response},
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

    let (token, mut game_rating, mut puzzle_rating) = {
        let conn = state.db.lock().unwrap();
        let token: Option<String> = conn
            .query_row(
                "SELECT access_token FROM lichess_tokens WHERE username = ?1",
                params![username],
                |row| row.get(0),
            )
            .ok();

        let mut g_rate = 1500;
        let mut p_rate = 1500;
        if let Some(user) = db::get_user(&conn, &username).unwrap() {
            g_rate = user.current_game_rating;
            p_rate = user.current_puzzle_rating;
        }
        (token, g_rate, p_rate)
    };

    // 1. Fetch user's rating profile
    if let Some(ref t) = token {
        if let Ok(p) = lichess::fetch_profile(t).await {
            game_rating = p.perfs.blitz.or(p.perfs.rapid).map(|x| x.rating).unwrap_or(1500);
            puzzle_rating = p.perfs.puzzle.map(|x| x.rating).unwrap_or(1500);
            let conn = state.db.lock().unwrap();
            let _ = db::update_user_ratings(&conn, &username, game_rating, puzzle_rating);
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
    let mut spins_earned_from_puzzles = 0;
    let mut total_eligible = 0;
    {
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
        total_eligible = eligible_puzzle_ids.len();
        let num_spins = total_eligible / 25;

        for i in 0..num_spins {
            // Claim 25 puzzles
            for j in 0..25 {
                let p_id = &eligible_puzzle_ids[i * 25 + j];
                let _ = db::claim_puzzle(&conn, &username, p_id);
            }
            let _ = db::add_spins(&conn, &username, 1);
            spins_earned_from_puzzles += 1;
        }
    }

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

pub async fn get_friends(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let username = match get_username(&headers) {
        Some(name) => name,
        None => return Err((StatusCode::UNAUTHORIZED, Json(serde_json::json!({ "error": "Not logged in" })))),
    };

    let conn = state.db.lock().unwrap();
    let friend_names = db::get_friends(&conn, &username)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": e.to_string() }))))?;

    let mut friends_profiles = Vec::new();
    for f_name in friend_names {
        // Check if the friend exists in our local DB
        if let Some(f_user) = db::get_user(&conn, &f_name).unwrap_or(None) {
            let equipped = db::get_equipped(&conn, &f_name).unwrap_or_default();
            friends_profiles.push(FriendProfile {
                username: f_user.username,
                avatar_base: f_user.avatar_base,
                current_game_rating: f_user.current_game_rating,
                current_puzzle_rating: f_user.current_puzzle_rating,
                equipped,
                lichess_url: format!("https://lichess.org/@/{}", f_name),
            });
        } else {
            // Friend exists in table but not in users table yet (might be just a lichess username added)
            friends_profiles.push(FriendProfile {
                username: f_name.clone(),
                avatar_base: "kid_boy".to_string(), // default
                current_game_rating: 1500,
                current_puzzle_rating: 1500,
                equipped: EquippedItems::default(),
                lichess_url: format!("https://lichess.org/@/{}", f_name),
            });
        }
    }

    Ok(Json(friends_profiles))
}

#[derive(Deserialize)]
pub struct AddFriendRequest {
    pub friend_username: String,
}

pub async fn add_friend(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<AddFriendRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let username = match get_username(&headers) {
        Some(name) => name,
        None => return Err((StatusCode::UNAUTHORIZED, Json(serde_json::json!({ "error": "Not logged in" })))),
    };

    let friend_username = payload.friend_username.trim();
    if friend_username.is_empty() {
        return Err((StatusCode::BAD_REQUEST, Json(serde_json::json!({ "error": "Friend username cannot be empty" }))));
    }

    if friend_username.to_lowercase() == username.to_lowercase() {
        return Err((StatusCode::BAD_REQUEST, Json(serde_json::json!({ "error": "You cannot add yourself as a friend!" }))));
    }

    let conn = state.db.lock().unwrap();

    // Verify friend exists or create placeholder
    // In a production app, we could call Lichess API to verify.
    // For this app, we will add them and show a success.
    match db::add_friend(&conn, &username, friend_username) {
        Ok(true) => {
            // If the user doesn't exist in local database, make a placeholder profile
            let _ = conn.execute(
                "INSERT OR IGNORE INTO users (username, avatar_base, current_game_rating, current_puzzle_rating) VALUES (?1, 'kid_boy', 1500, 1500)",
                params![friend_username],
            );
            let _ = conn.execute("INSERT OR IGNORE INTO equipped (username) VALUES (?1)", params![friend_username]);
            
            Ok(Json(serde_json::json!({ "success": true, "message": format!("Friend {} added successfully!", friend_username) })))
        }
        Ok(false) => Err((StatusCode::BAD_REQUEST, Json(serde_json::json!({ "error": "User is already your friend!" })))),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": e.to_string() })))),
    }
}

#[derive(Deserialize)]
pub struct DeleteFriendRequest {
    pub friend_username: String,
}

pub async fn delete_friend(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<DeleteFriendRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let username = match get_username(&headers) {
        Some(name) => name,
        None => return Err((StatusCode::UNAUTHORIZED, Json(serde_json::json!({ "error": "Not logged in" })))),
    };

    let conn = state.db.lock().unwrap();
    match db::delete_friend(&conn, &username, &payload.friend_username) {
        Ok(true) => Ok(Json(serde_json::json!({ "success": true, "message": "Friend removed" }))),
        Ok(false) => Err((StatusCode::BAD_REQUEST, Json(serde_json::json!({ "error": "User was not your friend" })))),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": e.to_string() })))),
    }
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
