use rusqlite::{params, Connection, Result};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct UserProfile {
    pub username: String,
    pub avatar_base: String,
    pub coins: i32,
    pub spins_available: i32,
    pub current_game_rating: i32,
    pub current_puzzle_rating: i32,
    pub last_daily_spin_claim: String,
    pub last_synced_at: i64,
    pub last_game_sync: i64,
    pub last_puzzle_sync: i64,
    pub total_games_claimed: i32,
    pub total_puzzles_claimed: i32,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct EquippedItems {
    pub top: Option<String>,
    pub bottom: Option<String>,
    pub hat: Option<String>,
    pub hair: Option<String>,
    pub accessory: Option<String>,
    pub background: Option<String>,
}

pub fn init_db(db_path: &str) -> Result<Connection> {
    let conn = Connection::open(db_path)?;

    // Enable foreign keys
    conn.execute("PRAGMA foreign_keys = ON;", [])?;

    // Create users table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS users (
            username TEXT PRIMARY KEY,
            avatar_base TEXT NOT NULL,
            coins INTEGER NOT NULL DEFAULT 0,
            spins_available INTEGER NOT NULL DEFAULT 0,
            current_game_rating INTEGER NOT NULL DEFAULT 1500,
            current_puzzle_rating INTEGER NOT NULL DEFAULT 1500,
            last_synced_at INTEGER NOT NULL DEFAULT 0,
            last_game_sync INTEGER NOT NULL DEFAULT 0,
            last_puzzle_sync INTEGER NOT NULL DEFAULT 0
        );",
        [],
    )?;

    // Create claimed games table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS claimed_games (
            username TEXT,
            game_id TEXT,
            PRIMARY KEY (username, game_id),
            FOREIGN KEY (username) REFERENCES users(username) ON DELETE CASCADE
        );",
        [],
    )?;

    // Create claimed puzzles table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS claimed_puzzles (
            username TEXT,
            puzzle_id TEXT,
            PRIMARY KEY (username, puzzle_id),
            FOREIGN KEY (username) REFERENCES users(username) ON DELETE CASCADE
        );",
        [],
    )?;

    // Create inventory table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS inventory (
            username TEXT,
            item_id TEXT NOT NULL,
            PRIMARY KEY (username, item_id),
            FOREIGN KEY (username) REFERENCES users(username) ON DELETE CASCADE
        );",
        [],
    )?;

    // Create equipped table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS equipped (
            username TEXT PRIMARY KEY,
            top TEXT,
            bottom TEXT,
            hat TEXT,
            hair TEXT,
            accessory TEXT,
            background TEXT,
            FOREIGN KEY (username) REFERENCES users(username) ON DELETE CASCADE
        );",
        [],
    )?;

    // Create friends table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS friends (
            username TEXT,
            friend_username TEXT,
            PRIMARY KEY (username, friend_username),
            FOREIGN KEY (username) REFERENCES users(username) ON DELETE CASCADE
        );",
        [],
    )?;

    // Migration: Add last_daily_spin_claim column to users if it doesn't exist
    let _ = conn.execute("ALTER TABLE users ADD COLUMN last_daily_spin_claim TEXT DEFAULT '';", []);

    // Migration: Add last_synced_at column to users if it doesn't exist
    let _ = conn.execute("ALTER TABLE users ADD COLUMN last_synced_at INTEGER DEFAULT 0;", []);

    // Migration: Add last_game_sync and last_puzzle_sync columns to users if they don't exist
    let _ = conn.execute("ALTER TABLE users ADD COLUMN last_game_sync INTEGER DEFAULT 0;", []);
    let _ = conn.execute("ALTER TABLE users ADD COLUMN last_puzzle_sync INTEGER DEFAULT 0;", []);

    Ok(conn)
}

pub fn create_user(conn: &Connection, username: &str, avatar_base: &str) -> Result<()> {
    conn.execute(
        "INSERT OR IGNORE INTO users (username, avatar_base, coins, spins_available) VALUES (?1, ?2, 0, 0)",
        params![username, avatar_base],
    )?;

    conn.execute(
        "INSERT OR IGNORE INTO equipped (username) VALUES (?1)",
        params![username],
    )?;

    Ok(())
}

pub fn get_user(conn: &Connection, username: &str) -> Result<Option<UserProfile>> {
    let mut stmt = conn.prepare(
        "SELECT username, avatar_base, coins, spins_available, current_game_rating, current_puzzle_rating, last_daily_spin_claim, last_synced_at, last_game_sync, last_puzzle_sync 
         FROM users WHERE username = ?1"
    )?;

    let mut rows = stmt.query(params![username])?;
    if let Some(row) = rows.next()? {
        let total_games_claimed: i32 = conn.query_row(
            "SELECT COUNT(*) FROM claimed_games WHERE username = ?1",
            params![username],
            |row| row.get(0)
        ).unwrap_or(0);

        let total_puzzles_claimed: i32 = conn.query_row(
            "SELECT COUNT(*) FROM claimed_puzzles WHERE username = ?1",
            params![username],
            |row| row.get(0)
        ).unwrap_or(0);

        Ok(Some(UserProfile {
            username: row.get(0)?,
            avatar_base: row.get(1)?,
            coins: row.get(2)?,
            spins_available: row.get(3)?,
            current_game_rating: row.get(4)?,
            current_puzzle_rating: row.get(5)?,
            last_daily_spin_claim: row.get(6).unwrap_or_default(),
            last_synced_at: row.get(7).unwrap_or(0),
            last_game_sync: row.get(8).unwrap_or(0),
            last_puzzle_sync: row.get(9).unwrap_or(0),
            total_games_claimed,
            total_puzzles_claimed,
        }))
    } else {
        Ok(None)
    }
}

pub fn update_last_daily_spin_claim(conn: &Connection, username: &str, claim_date: &str) -> Result<()> {
    conn.execute(
        "UPDATE users SET last_daily_spin_claim = ?2 WHERE username = ?1",
        params![username, claim_date],
    )?;
    Ok(())
}

pub fn update_user_ratings(conn: &Connection, username: &str, game_rating: i32, puzzle_rating: i32) -> Result<()> {
    conn.execute(
        "UPDATE users SET current_game_rating = ?2, current_puzzle_rating = ?3 WHERE username = ?1",
        params![username, game_rating, puzzle_rating],
    )?;
    Ok(())
}

pub fn add_spins(conn: &Connection, username: &str, amount: i32) -> Result<i32> {
    conn.execute(
        "UPDATE users SET spins_available = spins_available + ?2 WHERE username = ?1",
        params![username, amount],
    )?;
    
    let mut stmt = conn.prepare("SELECT spins_available FROM users WHERE username = ?1")?;
    let spins: i32 = stmt.query_row(params![username], |r| r.get(0))?;
    Ok(spins)
}

pub fn use_spin(conn: &Connection, username: &str) -> Result<bool> {
    let mut stmt = conn.prepare("SELECT spins_available FROM users WHERE username = ?1")?;
    let spins: i32 = stmt.query_row(params![username], |r| r.get(0))?;
    if spins <= 0 {
        return Ok(false);
    }

    conn.execute(
        "UPDATE users SET spins_available = spins_available - 1 WHERE username = ?1",
        params![username],
    )?;
    Ok(true)
}

pub fn reward_coins(conn: &Connection, username: &str, amount: i32) -> Result<i32> {
    conn.execute(
        "UPDATE users SET coins = coins + ?2 WHERE username = ?1",
        params![username, amount],
    )?;
    
    let mut stmt = conn.prepare("SELECT coins FROM users WHERE username = ?1")?;
    let coins: i32 = stmt.query_row(params![username], |r| r.get(0))?;
    Ok(coins)
}

pub fn is_game_claimed(conn: &Connection, username: &str, game_id: &str) -> Result<bool> {
    let mut stmt = conn.prepare("SELECT COUNT(*) FROM claimed_games WHERE username = ?1 AND game_id = ?2")?;
    let count: i64 = stmt.query_row(params![username, game_id], |r| r.get(0))?;
    Ok(count > 0)
}

pub fn claim_game(conn: &Connection, username: &str, game_id: &str) -> Result<bool> {
    let rows_affected = conn.execute(
        "INSERT OR IGNORE INTO claimed_games (username, game_id) VALUES (?1, ?2)",
        params![username, game_id],
    )?;
    Ok(rows_affected > 0)
}

pub fn claim_puzzle(conn: &Connection, username: &str, puzzle_id: &str) -> Result<bool> {
    let rows_affected = conn.execute(
        "INSERT OR IGNORE INTO claimed_puzzles (username, puzzle_id) VALUES (?1, ?2)",
        params![username, puzzle_id],
    )?;
    Ok(rows_affected > 0)
}

pub fn buy_item(conn: &Connection, username: &str, item_id: &str, cost: i32) -> Result<Result<i32, String>> {
    // Check if user already owns it
    let mut stmt = conn.prepare("SELECT COUNT(*) FROM inventory WHERE username = ?1 AND item_id = ?2")?;
    let count: i64 = stmt.query_row(params![username, item_id], |r| r.get(0))?;
    if count > 0 {
        return Ok(Err("You already own this item!".to_string()));
    }

    // Check coins
    let mut stmt = conn.prepare("SELECT coins FROM users WHERE username = ?1")?;
    let coins: i32 = stmt.query_row(params![username], |r| r.get(0))?;
    if coins < cost {
        return Ok(Err(format!("Not enough coins! Need {}, but only have {}", cost, coins)));
    }

    // Deduct coins and add to inventory (transaction or block)
    conn.execute(
        "UPDATE users SET coins = coins - ?2 WHERE username = ?1",
        params![username, cost],
    )?;

    conn.execute(
        "INSERT INTO inventory (username, item_id) VALUES (?1, ?2)",
        params![username, item_id],
    )?;

    let new_coins = coins - cost;
    Ok(Ok(new_coins))
}

pub fn get_inventory(conn: &Connection, username: &str) -> Result<Vec<String>> {
    let mut stmt = conn.prepare("SELECT item_id FROM inventory WHERE username = ?1")?;
    let rows = stmt.query_map(params![username], |row| row.get(0))?;
    let mut items = Vec::new();
    for row in rows {
        items.push(row?);
    }
    Ok(items)
}

pub fn get_equipped(conn: &Connection, username: &str) -> Result<EquippedItems> {
    let mut stmt = conn.prepare(
        "SELECT top, bottom, hat, hair, accessory, background FROM equipped WHERE username = ?1"
    )?;
    let mut rows = stmt.query(params![username])?;
    if let Some(row) = rows.next()? {
        Ok(EquippedItems {
            top: row.get(0)?,
            bottom: row.get(1)?,
            hat: row.get(2)?,
            hair: row.get(3)?,
            accessory: row.get(4)?,
            background: row.get(5)?,
        })
    } else {
        Ok(EquippedItems::default())
    }
}

pub fn equip_item(conn: &Connection, username: &str, category: &str, item_id: Option<&str>) -> Result<Result<(), String>> {
    // If equipping an item, verify user owns it (except if None, which means unequipping)
    if let Some(id) = item_id {
        let mut stmt = conn.prepare("SELECT COUNT(*) FROM inventory WHERE username = ?1 AND item_id = ?2")?;
        let count: i64 = stmt.query_row(params![username, id], |r| r.get(0))?;
        if count == 0 {
            return Ok(Err("You do not own this item!".to_string()));
        }
    }

    let query = match category {
        "top" => "UPDATE equipped SET top = ?2 WHERE username = ?1",
        "bottom" => "UPDATE equipped SET bottom = ?2 WHERE username = ?1",
        "hat" => "UPDATE equipped SET hat = ?2 WHERE username = ?1",
        "hair" => "UPDATE equipped SET hair = ?2 WHERE username = ?1",
        "accessory" => "UPDATE equipped SET accessory = ?2 WHERE username = ?1",
        "background" => "UPDATE equipped SET background = ?2 WHERE username = ?1",
        _ => return Ok(Err("Invalid item category!".to_string())),
    };

    conn.execute(query, params![username, item_id])?;
    Ok(Ok(()))
}

pub fn get_friends(conn: &Connection, username: &str) -> Result<Vec<String>> {
    let mut stmt = conn.prepare("SELECT friend_username FROM friends WHERE username = ?1")?;
    let rows = stmt.query_map(params![username], |row| row.get(0))?;
    let mut friends = Vec::new();
    for row in rows {
        friends.push(row?);
    }
    Ok(friends)
}

pub fn add_friend(conn: &Connection, username: &str, friend_username: &str) -> Result<bool> {
    let rows_affected = conn.execute(
        "INSERT OR IGNORE INTO friends (username, friend_username) VALUES (?1, ?2)",
        params![username, friend_username],
    )?;
    Ok(rows_affected > 0)
}

pub fn delete_friend(conn: &Connection, username: &str, friend_username: &str) -> Result<bool> {
    let rows_affected = conn.execute(
        "DELETE FROM friends WHERE username = ?1 AND friend_username = ?2",
        params![username, friend_username],
    )?;
    Ok(rows_affected > 0)
}

pub fn update_last_synced_at(conn: &Connection, username: &str, last_synced_at: i64) -> Result<()> {
    conn.execute(
        "UPDATE users SET last_synced_at = ?2 WHERE username = ?1",
        params![username, last_synced_at],
    )?;
    Ok(())
}

pub fn update_sync_timestamps(conn: &Connection, username: &str, last_game_sync: i64, last_puzzle_sync: i64) -> Result<()> {
    conn.execute(
        "UPDATE users SET last_game_sync = ?2, last_puzzle_sync = ?3 WHERE username = ?1",
        params![username, last_game_sync, last_puzzle_sync],
    )?;
    Ok(())
}

pub fn delete_user(conn: &Connection, username: &str) -> Result<()> {
    conn.execute("DELETE FROM users WHERE username = ?1", params![username])?;
    conn.execute("DELETE FROM lichess_tokens WHERE username = ?1", params![username])?;
    Ok(())
}




