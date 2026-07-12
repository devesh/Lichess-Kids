use lichesskids::db;

fn setup_in_memory_db() -> rusqlite::Connection {
    db::init_db(":memory:").expect("Failed to initialize in-memory DB")
}

#[test]
fn test_create_and_get_user() {
    let conn = setup_in_memory_db();
    
    // Check user doesn't exist initially
    let user = db::get_user(&conn, "bob").unwrap();
    assert!(user.is_none());

    // Create user
    db::create_user(&conn, "bob", "cat").unwrap();

    // Retrieve user and assert details
    let user = db::get_user(&conn, "bob").unwrap().unwrap();
    assert_eq!(user.username, "bob");
    assert_eq!(user.avatar_base, "cat");
    assert_eq!(user.coins, 0);
    assert_eq!(user.spins_available, 0);
}

#[test]
fn test_ratings_and_coins() {
    let conn = setup_in_memory_db();
    db::create_user(&conn, "alice", "dog").unwrap();

    // Reward coins
    let new_coins = db::reward_coins(&conn, "alice", 15).unwrap();
    assert_eq!(new_coins, 15);
    let user = db::get_user(&conn, "alice").unwrap().unwrap();
    assert_eq!(user.coins, 15);
}

#[test]
fn test_spins_claim() {
    let conn = setup_in_memory_db();
    db::create_user(&conn, "charlie", "alien").unwrap();

    // Default spins
    let user = db::get_user(&conn, "charlie").unwrap().unwrap();
    assert_eq!(user.spins_available, 0);

    // Add spins
    let spins = db::add_spins(&conn, "charlie", 2).unwrap();
    assert_eq!(spins, 2);

    // Claim game (first time succeeds, second time fails)
    let first_claim = db::claim_game(&conn, "charlie", "game_123").unwrap();
    assert!(first_claim);

    let second_claim = db::claim_game(&conn, "charlie", "game_123").unwrap();
    assert!(!second_claim);

    // Use spin
    let used = db::use_spin(&conn, "charlie").unwrap();
    assert!(used);
    let user = db::get_user(&conn, "charlie").unwrap().unwrap();
    assert_eq!(user.spins_available, 1);

    let used = db::use_spin(&conn, "charlie").unwrap();
    assert!(used);
    let used_fail = db::use_spin(&conn, "charlie").unwrap();
    assert!(!used_fail); // 0 spins left
}

#[test]
fn test_shop_and_equip() {
    let conn = setup_in_memory_db();
    db::create_user(&conn, "david", "cat").unwrap();

    // Try to buy hoodie with 0 coins - should fail
    let buy_res = db::buy_item(&conn, "david", "hoodie", 15).unwrap();
    assert!(buy_res.is_err());

    // Reward coins and buy
    db::reward_coins(&conn, "david", 20).unwrap();
    let buy_res = db::buy_item(&conn, "david", "hoodie", 15).unwrap().unwrap();
    assert_eq!(buy_res, 5); // 20 - 15 = 5 coins left

    // Verify item in inventory
    let inv = db::get_inventory(&conn, "david").unwrap();
    assert!(inv.contains(&"hoodie".to_string()));

    // Try to buy it again - should fail since user already owns it
    let rebuy_res = db::buy_item(&conn, "david", "hoodie", 15).unwrap();
    assert!(rebuy_res.is_err());

    // Equip owned item
    let equip_res = db::equip_item(&conn, "david", "top", Some("hoodie")).unwrap();
    assert!(equip_res.is_ok());

    // Equip unowned item - should fail
    let equip_fail = db::equip_item(&conn, "david", "hat", Some("crown")).unwrap();
    assert!(equip_fail.is_err());

    // Verify equipped items
    let equipped = db::get_equipped(&conn, "david").unwrap();
    assert_eq!(equipped.top, Some("hoodie".to_string()));
    assert_eq!(equipped.hat, None);
}

#[test]
fn test_friends() {
    let conn = setup_in_memory_db();
    db::create_user(&conn, "emma", "dog").unwrap();

    // Add friend
    let added = db::add_friend(&conn, "emma", "frank").unwrap();
    assert!(added);

    // Try adding duplicate
    let added_dup = db::add_friend(&conn, "emma", "frank").unwrap();
    assert!(!added_dup);

    // Get friends list
    let friends = db::get_friends(&conn, "emma").unwrap();
    assert_eq!(friends.len(), 1);
    assert_eq!(friends[0], "frank");

    // Delete friend
    let deleted = db::delete_friend(&conn, "emma", "frank").unwrap();
    assert!(deleted);

    let friends = db::get_friends(&conn, "emma").unwrap();
    assert_eq!(friends.len(), 0);
}



#[test]
fn test_asset_catalog() {
    let assets = lichesskids::assets::AssetCatalog::load_from_dir("assets").unwrap();
    assert!(!assets.bases.is_empty());
    assert!(!assets.items.is_empty());
    assert!(assets.bases_map.contains_key("cat"));
    assert!(assets.items_map.contains_key("party_hat"));
}

#[test]
fn test_spin_rules_and_daily_spin() {
    let conn = setup_in_memory_db();
    db::create_user(&conn, "gabriel", "cat").unwrap();

    let u = db::get_user(&conn, "gabriel").unwrap().unwrap();
    assert_eq!(u.last_daily_spin_claim, "");

    db::update_last_daily_spin_claim(&conn, "gabriel", "2026-07-05").unwrap();
    let u2 = db::get_user(&conn, "gabriel").unwrap().unwrap();
    assert_eq!(u2.last_daily_spin_claim, "2026-07-05");
}

#[test]
fn test_last_synced_at() {
    let conn = setup_in_memory_db();
    db::create_user(&conn, "gabriel", "cat").unwrap();

        let u = db::get_user(&conn, "gabriel").unwrap().unwrap();
    assert_eq!(u.last_synced_at, 0);
    assert_eq!(u.last_game_sync, 0);

    db::update_last_synced_at(&conn, "gabriel", 123456789).unwrap();
    db::update_sync_timestamps(&conn, "gabriel", 987654321).unwrap();
    let u2 = db::get_user(&conn, "gabriel").unwrap().unwrap();
    assert_eq!(u2.last_synced_at, 123456789);
    assert_eq!(u2.last_game_sync, 987654321);
}

#[test]
fn test_delete_user() {
    let conn = setup_in_memory_db();
    db::create_user(&conn, "gabriel", "cat").unwrap();
    
    conn.execute(
        "CREATE TABLE IF NOT EXISTS lichess_tokens (username TEXT PRIMARY KEY, access_token TEXT NOT NULL);",
        []
    ).unwrap();
    conn.execute(
        "INSERT INTO lichess_tokens (username, access_token) VALUES ('gabriel', 'my_token');",
        []
    ).unwrap();

    let u = db::get_user(&conn, "gabriel").unwrap();
    assert!(u.is_some());

    db::delete_user(&conn, "gabriel").unwrap();
    let u_after = db::get_user(&conn, "gabriel").unwrap();
    assert!(u_after.is_none());

    let token_exists: i64 = conn.query_row(
        "SELECT COUNT(*) FROM lichess_tokens WHERE username = 'gabriel'",
        [],
        |row| row.get(0)
    ).unwrap();
    assert_eq!(token_exists, 0);
}

#[test]
fn test_sync_puzzle_high_scores() {
    let conn = setup_in_memory_db();
    db::create_user(&conn, "henry", "cat").unwrap();

    // First sync: award spins equal to the current high scores (incl. duel/racer).
    let (streak, storm, racer) = db::sync_puzzle_high_scores(&conn, "henry", 10, 25, 5).unwrap();
    assert_eq!(streak, 10);
    assert_eq!(storm, 25);
    assert_eq!(racer, 5);
    let u = db::get_user(&conn, "henry").unwrap().unwrap();
    assert_eq!(u.spins_available, 40);

    // Second sync: scores unchanged -> no additional spins.
    let (streak2, storm2, racer2) = db::sync_puzzle_high_scores(&conn, "henry", 10, 25, 5).unwrap();
    assert_eq!(streak2, 0);
    assert_eq!(storm2, 0);
    assert_eq!(racer2, 0);
    let u2 = db::get_user(&conn, "henry").unwrap().unwrap();
    assert_eq!(u2.spins_available, 40);

    // Third sync: only storm improved -> award only the storm delta.
    let (streak3, storm3, racer3) = db::sync_puzzle_high_scores(&conn, "henry", 10, 40, 5).unwrap();
    assert_eq!(streak3, 0);
    assert_eq!(storm3, 15);
    assert_eq!(racer3, 0);
    let u3 = db::get_user(&conn, "henry").unwrap().unwrap();
    assert_eq!(u3.spins_available, 55);
    // Total awarded equals the highest scores for each mode.
    assert_eq!(
        u3.spins_available,
        u3.last_puzzle_streak_score + u3.last_puzzle_storm_score + u3.last_puzzle_racer_score
    );
}

#[test]
fn test_sync_puzzle_high_scores_edge_cases() {
    let conn = setup_in_memory_db();
    db::create_user(&conn, "ivy", "cat").unwrap();

    // All modes improve at once -> award the full delta for each.
    let (streak, storm, racer) = db::sync_puzzle_high_scores(&conn, "ivy", 7, 12, 3).unwrap();
    assert_eq!(streak, 7);
    assert_eq!(storm, 12);
    assert_eq!(racer, 3);

    // A score dropping below the stored high must NOT award negative spins
    // and must NOT lower the stored high (lifetime total stays correct).
    let (streak_down, storm_down, racer_down) = db::sync_puzzle_high_scores(&conn, "ivy", 3, 9, 1).unwrap();
    assert_eq!(streak_down, 0);
    assert_eq!(storm_down, 0);
    assert_eq!(racer_down, 0);
    let u = db::get_user(&conn, "ivy").unwrap().unwrap();
    assert_eq!(u.last_puzzle_streak_score, 7);
    assert_eq!(u.last_puzzle_storm_score, 12);
    assert_eq!(u.last_puzzle_racer_score, 3);
    assert_eq!(u.spins_available, 22);

    // Repeated identical syncs grant nothing further.
    for _ in 0..3 {
        let (s, st, r) = db::sync_puzzle_high_scores(&conn, "ivy", 7, 12, 3).unwrap();
        assert_eq!(s, 0);
        assert_eq!(st, 0);
        assert_eq!(r, 0);
    }

    // Incremental improvements across many syncs accumulate to the final high score.
    let (s1, st1, r1) = db::sync_puzzle_high_scores(&conn, "ivy", 20, 12, 3).unwrap();
    assert_eq!(s1, 13);
    assert_eq!(st1, 0);
    assert_eq!(r1, 0);
    let (s2, st2, r2) = db::sync_puzzle_high_scores(&conn, "ivy", 20, 50, 30).unwrap();
    assert_eq!(s2, 0);
    assert_eq!(st2, 38);
    assert_eq!(r2, 27);

    let u_final = db::get_user(&conn, "ivy").unwrap().unwrap();
    // Total spins awarded always equals the sum of the current high scores.
    assert_eq!(
        u_final.spins_available,
        u_final.last_puzzle_streak_score + u_final.last_puzzle_storm_score + u_final.last_puzzle_racer_score
    );
    assert_eq!(u_final.spins_available, 100);
}

