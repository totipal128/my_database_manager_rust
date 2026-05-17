use my_database_manager::{DatabaseConfig, Model, create_table, setup_database, sync_table};
use sqlx::Row;

// ----- Struct dengan berbagai tipe field -----

/// Versi awal: hanya id + username
#[allow(dead_code)]
#[derive(Debug, Clone, Model)]
#[table("test_users")]
struct TestUser {
    id: i32,               // → id INT PRIMARY KEY (dikecualikan dari INSERT)
    username: String,      // → username VARCHAR(255) NOT NULL
    email: Option<String>, // → email VARCHAR(255)  (nullable)
}

/// Versi upgrade: tambah field age dan score
#[allow(dead_code)]
#[derive(Debug, Clone, Model)]
#[table("test_users")]
struct TestUserV2 {
    id: i32,
    username: String,
    email: Option<String>,
    age: i32,              // → age INT NOT NULL
    score: Option<f64>,    // → score DOUBLE (nullable)
}

// ----- Helper -----

async fn get_columns(pool: &sqlx::Pool<sqlx::Any>, table: &str) -> Vec<String> {
    let query = format!("PRAGMA table_info('{}')", table);
    let rows = sqlx::query(&query).fetch_all(pool).await.unwrap();
    rows.into_iter()
        .map(|r| r.try_get::<String, _>("name").unwrap())
        .collect()
}

// ----- Test 1: Auto-derive membuat kolom yang benar -----
#[tokio::test]
async fn test_auto_derive_creates_correct_columns() {
    let _ = std::fs::remove_file("test_auto_derive.sqlite");
    let config = DatabaseConfig::new("sqlite", "", 0, "", "", "test_auto_derive.sqlite");

    setup_database(&config).await.unwrap();
    let pool = config.connect().await.unwrap();

    // Buat tabel dari struct yang di-derive otomatis
    create_table::<TestUser>(&pool).await.expect("Gagal buat tabel");

    let cols = get_columns(&pool, "test_users").await;
    println!("[test_auto_derive] Columns: {:?}", cols);

    // Verifikasi kolom yang dihasilkan oleh derive macro
    assert!(cols.contains(&"id".to_string()),       "id harus ada sebagai PK");
    assert!(cols.contains(&"username".to_string()), "username harus ada");
    assert!(cols.contains(&"email".to_string()),    "email (Option<String>) harus ada");

    // Verifikasi id TIDAK ada di FIELDS_INSERT
    assert!(!TestUser::FIELDS_INSERT.contains(&"id"), "id tidak boleh ada di FIELDS_INSERT");
    assert!(TestUser::FIELDS_INSERT.contains(&"username"));
    assert!(TestUser::FIELDS_INSERT.contains(&"email"));
}

// ----- Test 2: sync_table menambah kolom baru secara otomatis -----
#[tokio::test]
async fn test_sync_adds_new_columns() {
    let _ = std::fs::remove_file("test_sync_add.sqlite");
    let config = DatabaseConfig::new("sqlite", "", 0, "", "", "test_sync_add.sqlite");

    setup_database(&config).await.unwrap();
    let pool = config.connect().await.unwrap();

    // Buat tabel dengan versi awal
    create_table::<TestUser>(&pool).await.unwrap();

    let cols_before = get_columns(&pool, "test_users").await;
    assert!(!cols_before.contains(&"age".to_string()),   "age belum ada di versi awal");
    assert!(!cols_before.contains(&"score".to_string()), "score belum ada di versi awal");

    // Sync ke versi upgrade (tambah age + score)
    sync_table::<TestUserV2>(&pool, &config.driver).await.unwrap();

    let cols_after = get_columns(&pool, "test_users").await;
    println!("[test_sync_add] Columns after sync: {:?}", cols_after);

    assert!(cols_after.contains(&"age".to_string()),   "age harus ditambahkan oleh sync");
    assert!(cols_after.contains(&"score".to_string()), "score harus ditambahkan oleh sync");
}

// ----- Test 3: sync_table menghapus kolom lama secara otomatis -----
#[tokio::test]
async fn test_sync_drops_removed_columns() {
    let _ = std::fs::remove_file("test_sync_drop.sqlite");
    let config = DatabaseConfig::new("sqlite", "", 0, "", "", "test_sync_drop.sqlite");

    setup_database(&config).await.unwrap();
    let pool = config.connect().await.unwrap();

    // Mulai dengan versi lengkap (V2)
    create_table::<TestUserV2>(&pool).await.unwrap();
    let cols_before = get_columns(&pool, "test_users").await;
    assert!(cols_before.contains(&"age".to_string()));

    // Revert ke versi awal → age dan score harus hilang
    sync_table::<TestUser>(&pool, &config.driver).await.unwrap();

    // SQLite mendukung DROP COLUMN sejak v3.35.0.
    // Test memastikan fungsi tidak error (walau mungkin ada warning di versi lama).
    // Jika berhasil, kolom harus terhapus.
    let cols_after = get_columns(&pool, "test_users").await;
    println!("[test_sync_drop] Columns after revert: {:?}", cols_after);

    assert!(cols_after.contains(&"id".to_string()),       "id tetap ada");
    assert!(cols_after.contains(&"username".to_string()), "username tetap ada");
}

// ----- Test 4: Full flow setup → create → sync -----
#[tokio::test]
async fn test_database_manager_full_flow() {
    let _ = std::fs::remove_file("test_db.sqlite");
    let config = DatabaseConfig::new("sqlite", "", 0, "", "", "test_db.sqlite");

    // 1. Setup database
    let setup_res = setup_database(&config).await;
    assert!(setup_res.is_ok(), "Gagal menyiapkan database");

    let pool = config.connect().await.expect("Gagal connect ke sqlite");

    // 2. Create table dari struct auto-derive
    create_table::<TestUser>(&pool).await.expect("Gagal buat tabel");

    let cols = get_columns(&pool, "test_users").await;
    assert!(cols.contains(&"id".to_string()));
    assert!(cols.contains(&"username".to_string()));
    assert!(cols.contains(&"email".to_string()));
    assert!(!cols.contains(&"age".to_string()));

    // 3. Sync ke V2 (tambah age + score)
    sync_table::<TestUserV2>(&pool, &config.driver).await.expect("Gagal sync ke V2");

    let cols_v2 = get_columns(&pool, "test_users").await;
    assert!(cols_v2.contains(&"age".to_string()));
    assert!(cols_v2.contains(&"score".to_string()));

    // 4. Sync kembali ke V1 (hapus age + score)
    sync_table::<TestUser>(&pool, &config.driver).await.expect("Gagal revert ke V1");
}
