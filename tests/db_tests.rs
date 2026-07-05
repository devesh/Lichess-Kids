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
    assert_eq!(user.current_game_rating, 1500);
    assert_eq!(user.current_puzzle_rating, 1500);
}

#[test]
fn test_ratings_and_coins() {
    let conn = setup_in_memory_db();
    db::create_user(&conn, "alice", "dog").unwrap();

    // Update ratings
    db::update_user_ratings(&conn, "alice", 1600, 1700).unwrap();
    let user = db::get_user(&conn, "alice").unwrap().unwrap();
    assert_eq!(user.current_game_rating, 1600);
    assert_eq!(user.current_puzzle_rating, 1700);

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

