use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LichessPerf {
    pub rating: i32,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LichessPuzzleHighScore {
    pub runs: i32,
    pub score: i32,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LichessPerfs {
    pub puzzle: Option<LichessPerf>,
    pub blitz: Option<LichessPerf>,
    pub bullet: Option<LichessPerf>,
    pub rapid: Option<LichessPerf>,
    pub storm: Option<LichessPuzzleHighScore>,
    pub streak: Option<LichessPuzzleHighScore>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LichessProfile {
    pub id: String,
    pub username: String,
    pub perfs: LichessPerfs,
    #[serde(rename = "createdAt")]
    pub created_at: Option<i64>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LichessPlayerUser {
    pub name: String,
    pub id: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LichessPlayer {
    pub user: Option<LichessPlayerUser>,
    pub rating: Option<i32>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LichessPlayers {
    pub white: LichessPlayer,
    pub black: LichessPlayer,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LichessGame {
    pub id: String,
    pub rated: bool,
    pub players: LichessPlayers,
    pub winner: Option<String>, // "white" or "black"
    #[serde(rename = "createdAt")]
    pub created_at: i64,
}



pub async fn fetch_profile(token: &str) -> Result<LichessProfile, reqwest::Error> {
    let client = reqwest::Client::new();
    let response = client
        .get("https://lichess.org/api/account")
        .bearer_auth(token)
        .header("User-Agent", "LichessKids-App/1.0")
        .send()
        .await?;

    response.error_for_status()?.json::<LichessProfile>().await
}

pub async fn fetch_games(username: &str, token: &str, query: &[(&str, String)]) -> Result<Vec<LichessGame>, reqwest::Error> {
    let client = reqwest::Client::new();
    let response = client
        .get(format!("https://lichess.org/api/games/user/{}", username))
        .query(query)
        .bearer_auth(token)
        .header("User-Agent", "LichessKids-App/1.0")
        .header("Accept", "application/x-ndjson")
        .send()
        .await?;
    
    let text = response.error_for_status()?.text().await?;
    
    let mut games = Vec::new();
    for line in text.lines() {
        if !line.trim().is_empty() {
            if let Ok(game) = serde_json::from_str::<LichessGame>(line) {
                games.push(game);
            }
        }
    }
    
    Ok(games)
}



#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LichessFollowedUser {
    pub id: String,
    pub username: String,
}

pub async fn fetch_following(token: &str) -> Result<Vec<String>, reqwest::Error> {
    let client = reqwest::Client::new();
    let response = client
        .get("https://lichess.org/api/rel/following")
        .bearer_auth(token)
        .header("User-Agent", "LichessKids-App/1.0")
        .header("Accept", "application/x-ndjson")
        .send()
        .await?;

    let text = response.error_for_status()?.text().await?;
    let mut followed = Vec::new();
    for line in text.lines() {
        if !line.trim().is_empty() {
            if let Ok(user) = serde_json::from_str::<LichessFollowedUser>(line) {
                followed.push(user.username);
            }
        }
    }
    Ok(followed)
}


