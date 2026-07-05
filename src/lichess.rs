use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LichessPerf {
    pub rating: i32,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LichessPerfs {
    pub puzzle: Option<LichessPerf>,
    pub blitz: Option<LichessPerf>,
    pub bullet: Option<LichessPerf>,
    pub rapid: Option<LichessPerf>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LichessProfile {
    pub id: String,
    pub username: String,
    pub perfs: LichessPerfs,
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

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LichessPuzzle {
    pub id: String,
    pub rating: i32,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LichessPuzzleRound {
    pub date: i64,
    pub win: bool,
    pub puzzle: LichessPuzzle,
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

pub async fn fetch_games(username: &str, token: Option<&str>, max_games: u32) -> Result<Vec<LichessGame>, reqwest::Error> {
    // If username is mock_user or starts with mock, return mock games
    if username.starts_with("mock_") || token.is_none() {
        return Ok(generate_mock_games(username));
    }

    let client = reqwest::Client::new();
    let mut req = client
        .get(format!("https://lichess.org/api/games/user/{}", username))
        .query(&[("max", max_games.to_string()), ("rated", "true".to_string())])
        .header("User-Agent", "LichessKids-App/1.0");

    if let Some(t) = token {
        req = req.bearer_auth(t);
    }

    let response = req.send().await?;
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

pub async fn fetch_puzzle_activity(token: &str, max_puzzles: u32) -> Result<Vec<LichessPuzzleRound>, reqwest::Error> {
    // If token is mock_token, return mock puzzle activities
    if token == "mock_token" {
        return Ok(generate_mock_puzzles());
    }

    let client = reqwest::Client::new();
    let response = client
        .get("https://lichess.org/api/puzzle/activity")
        .query(&[("max", max_puzzles.to_string())])
        .bearer_auth(token)
        .header("User-Agent", "LichessKids-App/1.0")
        .send()
        .await?;

    let text = response.error_for_status()?.text().await?;
    let mut rounds = Vec::new();
    for line in text.lines() {
        if !line.trim().is_empty() {
            if let Ok(round) = serde_json::from_str::<LichessPuzzleRound>(line) {
                rounds.push(round);
            }
        }
    }

    Ok(rounds)
}

// Mock data generators for local testing without Lichess OAuth configuration
fn generate_mock_games(username: &str) -> Vec<LichessGame> {
    vec![
        LichessGame {
            id: "game1".to_string(),
            rated: true,
            players: LichessPlayers {
                white: LichessPlayer {
                    user: Some(LichessPlayerUser {
                        name: username.to_string(),
                        id: username.to_lowercase(),
                    }),
                    rating: Some(1500),
                },
                black: LichessPlayer {
                    user: Some(LichessPlayerUser {
                        name: "StrongOpponent".to_string(),
                        id: "strongopponent".to_string(),
                    }),
                    rating: Some(1550),
                },
            },
            winner: Some("white".to_string()),
            created_at: 1672531100000,
        },
        LichessGame {
            id: "game2".to_string(),
            rated: true,
            players: LichessPlayers {
                white: LichessPlayer {
                    user: Some(LichessPlayerUser {
                        name: "EasyBot".to_string(),
                        id: "easybot".to_string(),
                    }),
                    rating: Some(1400),
                },
                black: LichessPlayer {
                    user: Some(LichessPlayerUser {
                        name: username.to_string(),
                        id: username.to_lowercase(),
                    }),
                    rating: Some(1510),
                },
            },
            winner: Some("black".to_string()), // User won
            created_at: 1672531200000,
        },
        LichessGame {
            id: "game3".to_string(),
            rated: true,
            players: LichessPlayers {
                white: LichessPlayer {
                    user: Some(LichessPlayerUser {
                        name: username.to_string(),
                        id: username.to_lowercase(),
                    }),
                    rating: Some(1520),
                },
                black: LichessPlayer {
                    user: Some(LichessPlayerUser {
                        name: "EvenOpponent".to_string(),
                        id: "evenopponent".to_string(),
                    }),
                    rating: Some(1520),
                },
            },
            winner: Some("white".to_string()), // User won
            created_at: 1672531300000,
        },
        LichessGame {
            id: "game4".to_string(),
            rated: true,
            players: LichessPlayers {
                white: LichessPlayer {
                    user: Some(LichessPlayerUser {
                        name: "ToughBot".to_string(),
                        id: "toughbot".to_string(),
                    }),
                    rating: Some(1600),
                },
                black: LichessPlayer {
                    user: Some(LichessPlayerUser {
                        name: username.to_string(),
                        id: username.to_lowercase(),
                    }),
                    rating: Some(1525),
                },
            },
            winner: Some("white".to_string()), // User lost
            created_at: 1672531400000,
        },
    ]
}

fn generate_mock_puzzles() -> Vec<LichessPuzzleRound> {
    vec![
        LichessPuzzleRound {
            date: 1672531100000,
            win: true,
            puzzle: LichessPuzzle {
                id: "puzzle1".to_string(),
                rating: 1600,
            },
        },
        LichessPuzzleRound {
            date: 1672531200000,
            win: true,
            puzzle: LichessPuzzle {
                id: "puzzle2".to_string(),
                rating: 1550,
            },
        },
        LichessPuzzleRound {
            date: 1672531300000,
            win: false, // lost
            puzzle: LichessPuzzle {
                id: "puzzle3".to_string(),
                rating: 1580,
            },
        },
        LichessPuzzleRound {
            date: 1672531400000,
            win: true,
            puzzle: LichessPuzzle {
                id: "puzzle4".to_string(),
                rating: 1450,
            },
        },
        // We'll generate 30 puzzles here so that user can claim 1 spin (since 25 correct rated puzzles are needed)
        // Let's generate 26 successful puzzles >= rating
        LichessPuzzleRound {
            date: 1672531500000,
            win: true,
            puzzle: LichessPuzzle { id: "p5".to_string(), rating: 1530 },
        },
        LichessPuzzleRound { date: 1672531600000, win: true, puzzle: LichessPuzzle { id: "p6".to_string(), rating: 1535 } },
        LichessPuzzleRound { date: 1672531700000, win: true, puzzle: LichessPuzzle { id: "p7".to_string(), rating: 1540 } },
        LichessPuzzleRound { date: 1672531800000, win: true, puzzle: LichessPuzzle { id: "p8".to_string(), rating: 1545 } },
        LichessPuzzleRound { date: 1672531900000, win: true, puzzle: LichessPuzzle { id: "p9".to_string(), rating: 1550 } },
        LichessPuzzleRound { date: 1672532000000, win: true, puzzle: LichessPuzzle { id: "p10".to_string(), rating: 1555 } },
        LichessPuzzleRound { date: 1672532100000, win: true, puzzle: LichessPuzzle { id: "p11".to_string(), rating: 1560 } },
        LichessPuzzleRound { date: 1672532200000, win: true, puzzle: LichessPuzzle { id: "p12".to_string(), rating: 1565 } },
        LichessPuzzleRound { date: 1672532300000, win: true, puzzle: LichessPuzzle { id: "p13".to_string(), rating: 1570 } },
        LichessPuzzleRound { date: 1672532400000, win: true, puzzle: LichessPuzzle { id: "p14".to_string(), rating: 1575 } },
        LichessPuzzleRound { date: 1672532500000, win: true, puzzle: LichessPuzzle { id: "p15".to_string(), rating: 1580 } },
        LichessPuzzleRound { date: 1672532600000, win: true, puzzle: LichessPuzzle { id: "p16".to_string(), rating: 1585 } },
        LichessPuzzleRound { date: 1672532700000, win: true, puzzle: LichessPuzzle { id: "p17".to_string(), rating: 1590 } },
        LichessPuzzleRound { date: 1672532800000, win: true, puzzle: LichessPuzzle { id: "p18".to_string(), rating: 1595 } },
        LichessPuzzleRound { date: 1672532900000, win: true, puzzle: LichessPuzzle { id: "p19".to_string(), rating: 1600 } },
        LichessPuzzleRound { date: 1672533000000, win: true, puzzle: LichessPuzzle { id: "p20".to_string(), rating: 1605 } },
        LichessPuzzleRound { date: 1672533100000, win: true, puzzle: LichessPuzzle { id: "p21".to_string(), rating: 1610 } },
        LichessPuzzleRound { date: 1672533200000, win: true, puzzle: LichessPuzzle { id: "p22".to_string(), rating: 1615 } },
        LichessPuzzleRound { date: 1672533300000, win: true, puzzle: LichessPuzzle { id: "p23".to_string(), rating: 1620 } },
        LichessPuzzleRound { date: 1672533400000, win: true, puzzle: LichessPuzzle { id: "p24".to_string(), rating: 1625 } },
        LichessPuzzleRound { date: 1672533500000, win: true, puzzle: LichessPuzzle { id: "p25".to_string(), rating: 1630 } },
        LichessPuzzleRound { date: 1672533600000, win: true, puzzle: LichessPuzzle { id: "p26".to_string(), rating: 1635 } },
        LichessPuzzleRound { date: 1672533700000, win: true, puzzle: LichessPuzzle { id: "p27".to_string(), rating: 1640 } },
        LichessPuzzleRound {
            date: 1672533800000,
            win: true,
            puzzle: LichessPuzzle { id: "p28".to_string(), rating: 1645 },
        },
        LichessPuzzleRound {
            date: 1672533900000,
            win: true,
            puzzle: LichessPuzzle { id: "p29".to_string(), rating: 1650 },
        },
        LichessPuzzleRound {
            date: 1672534000000,
            win: true,
            puzzle: LichessPuzzle { id: "p30".to_string(), rating: 1655 },
        },
    ]
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LichessFollowedUser {
    pub id: String,
    pub username: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LichessPublicProfileDetails {
    pub country: Option<String>,
    pub location: Option<String>,
    pub bio: Option<String>,
    pub links: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LichessPublicProfile {
    pub id: String,
    pub username: String,
    pub perfs: LichessPerfs,
    pub profile: Option<LichessPublicProfileDetails>,
}

pub async fn fetch_following(token: &str) -> Result<Vec<String>, reqwest::Error> {
    if token == "mock_token" {
        return Ok(vec![
            "strongopponent".to_string(),
            "easybot".to_string(),
            "evenopponent".to_string(),
        ]);
    }

    let client = reqwest::Client::new();
    let response = client
        .get("https://lichess.org/api/rel/following")
        .bearer_auth(token)
        .header("User-Agent", "LichessKids-App/1.0")
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

pub async fn fetch_public_profile(username: &str) -> Result<LichessPublicProfile, reqwest::Error> {
    if username == "strongopponent" || username == "easybot" || username == "evenopponent" {
        return Ok(LichessPublicProfile {
            id: username.to_string(),
            username: username.to_string(),
            perfs: LichessPerfs {
                puzzle: Some(LichessPerf { rating: 1550 }),
                blitz: Some(LichessPerf { rating: 1550 }),
                bullet: Some(LichessPerf { rating: 1500 }),
                rapid: Some(LichessPerf { rating: 1550 }),
            },
            profile: Some(LichessPublicProfileDetails {
                country: Some("US".to_string()),
                location: Some("Localhost".to_string()),
                bio: Some("Mock profile".to_string()),
                links: Some(format!("http://127.0.0.1:3000/user/{}", username)),
            }),
        });
    }

    let client = reqwest::Client::new();
    let response = client
        .get(format!("https://lichess.org/api/user/{}", username))
        .header("User-Agent", "LichessKids-App/1.0")
        .send()
        .await?;

    response.error_for_status()?.json::<LichessPublicProfile>().await
}

