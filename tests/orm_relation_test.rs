use my_database_manager::{
    DatabaseConfig, Model, OrmModel, WithChildren,
    create_table, setup_database, sync_table,
    insert, find_one, find_all,
    find_by, find_by_paginated,
    find_related, find_many_to_many,
    find_one_with_related,
};
use sqlx::{Row, any::AnyRow};

// ─────────────────────────────────────────────────────────────────────────────
// Model Structs
// ─────────────────────────────────────────────────────────────────────────────

#[allow(dead_code)]
#[derive(Debug, Clone, Model)]
#[table("rel_users")]
struct RelUser {
    id: i32,
    name: String,
}

impl OrmModel for RelUser {
    fn get_id(&self) -> i64 { self.id as i64 }
    fn insert_values(&self) -> Vec<String> { vec![self.name.clone()] }
    fn from_row(row: AnyRow) -> Result<Self, sqlx::Error> {
        Ok(RelUser { id: row.try_get("id")?, name: row.try_get("name")? })
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone, Model)]
#[table("rel_posts")]
struct RelPost {
    id: i32,
    title: String,
    user_id: i32,
}

impl OrmModel for RelPost {
    fn get_id(&self) -> i64 { self.id as i64 }
    fn insert_values(&self) -> Vec<String> {
        vec![self.title.clone(), self.user_id.to_string()]
    }
    fn from_row(row: AnyRow) -> Result<Self, sqlx::Error> {
        Ok(RelPost {
            id: row.try_get("id")?,
            title: row.try_get("title")?,
            user_id: row.try_get("user_id")?,
        })
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone, Model)]
#[table("rel_roles")]
struct RelRole {
    id: i32,
    name: String,
}

impl OrmModel for RelRole {
    fn get_id(&self) -> i64 { self.id as i64 }
    fn insert_values(&self) -> Vec<String> { vec![self.name.clone()] }
    fn from_row(row: AnyRow) -> Result<Self, sqlx::Error> {
        Ok(RelRole { id: row.try_get("id")?, name: row.try_get("name")? })
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Helper: Setup DB dengan skema relasional
// ─────────────────────────────────────────────────────────────────────────────

async fn setup_relational_db(file: &str) -> sqlx::Pool<sqlx::Any> {
    let _ = std::fs::remove_file(file);
    let config = DatabaseConfig::new("sqlite", "", 0, "", "", file);
    setup_database(&config).await.unwrap();
    let pool = config.connect().await.unwrap();

    // Buat tabel utama (urutan penting: parent dulu)
    create_table::<RelUser>(&pool).await.unwrap();
    create_table::<RelRole>(&pool).await.unwrap();

    // Buat tabel posts dengan kolom user_id secara manual (FK)
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS rel_posts (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            title TEXT NOT NULL,
            user_id INTEGER NOT NULL,
            FOREIGN KEY (user_id) REFERENCES rel_users(id)
        )"
    ).execute(&pool).await.unwrap();

    // Buat pivot table user_roles
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS rel_user_roles (
            user_id INTEGER NOT NULL,
            role_id INTEGER NOT NULL,
            PRIMARY KEY (user_id, role_id)
        )"
    ).execute(&pool).await.unwrap();

    pool
}

// ─────────────────────────────────────────────────────────────────────────────
// Test: FIND BY (filter kolom)
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_find_by_column() {
    let pool = setup_relational_db("test_rel_find_by.sqlite").await;

    // Seed users
    sqlx::query("INSERT INTO rel_users (id, name) VALUES (1, 'Alice'), (2, 'Bob')").execute(&pool).await.unwrap();
    // Seed posts: Alice = 3 post, Bob = 1 post
    for i in 1..=3 {
        sqlx::query("INSERT INTO rel_posts (id, title, user_id) VALUES (?, ?, 1)")
            .bind(i).bind(format!("Alice Post {}", i)).execute(&pool).await.unwrap();
    }
    sqlx::query("INSERT INTO rel_posts (id, title, user_id) VALUES (4, 'Bob Post 1', 2)").execute(&pool).await.unwrap();

    // find_by: filter user_id = 1
    let alice_posts = find_by::<RelPost>(&pool, "user_id", "1").await.unwrap();
    assert_eq!(alice_posts.len(), 3, "Alice harus punya 3 post");
    assert!(alice_posts.iter().all(|p| p.user_id == 1));

    // filter user_id = 2
    let bob_posts = find_by::<RelPost>(&pool, "user_id", "2").await.unwrap();
    assert_eq!(bob_posts.len(), 1, "Bob harus punya 1 post");
}

// ─────────────────────────────────────────────────────────────────────────────
// Test: FIND BY PAGINATED (filter dengan paginasi)
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_find_by_paginated() {
    let pool = setup_relational_db("test_rel_find_by_page.sqlite").await;

    // Seed: user 1 dengan 7 post
    sqlx::query("INSERT INTO rel_users (id, name) VALUES (1, 'Alice')").execute(&pool).await.unwrap();
    for i in 1..=7 {
        sqlx::query("INSERT INTO rel_posts (id, title, user_id) VALUES (?, ?, 1)")
            .bind(i).bind(format!("Post {}", i)).execute(&pool).await.unwrap();
    }

    // Halaman 1: 3 per halaman → total_pages = ceil(7/3) = 3
    let page1 = find_by_paginated::<RelPost>(&pool, "user_id", "1", 1, 3).await.unwrap();
    assert_eq!(page1.data.len(), 3);
    assert_eq!(page1.total, 7);
    assert_eq!(page1.total_pages, 3);

    // Halaman 3: hanya 1 data
    let page3 = find_by_paginated::<RelPost>(&pool, "user_id", "1", 3, 3).await.unwrap();
    assert_eq!(page3.data.len(), 1);
}

// ─────────────────────────────────────────────────────────────────────────────
// Test: FIND RELATED (One-to-Many)
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_find_related_one_to_many() {
    let pool = setup_relational_db("test_rel_one_to_many.sqlite").await;

    // Seed
    sqlx::query("INSERT INTO rel_users (id, name) VALUES (1, 'Alice')").execute(&pool).await.unwrap();
    for i in 1..=4 {
        sqlx::query("INSERT INTO rel_posts (id, title, user_id) VALUES (?, ?, 1)")
            .bind(i).bind(format!("Post {}", i)).execute(&pool).await.unwrap();
    }

    // find_related: semua post dengan user_id = 1
    let posts = find_related::<RelPost>(&pool, "user_id", 1).await.unwrap();
    assert_eq!(posts.len(), 4, "Alice harus punya 4 post");
    assert!(posts.iter().all(|p| p.user_id == 1));
}

// ─────────────────────────────────────────────────────────────────────────────
// Test: FIND ONE WITH RELATED (Parent + Children)
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_find_one_with_related() {
    let pool = setup_relational_db("test_rel_with_children.sqlite").await;

    // Seed
    sqlx::query("INSERT INTO rel_users (id, name) VALUES (1, 'Alice'), (2, 'Bob')").execute(&pool).await.unwrap();
    for i in 1..=3 {
        sqlx::query("INSERT INTO rel_posts (id, title, user_id) VALUES (?, ?, 1)")
            .bind(i).bind(format!("Alice Post {}", i)).execute(&pool).await.unwrap();
    }

    // find_one_with_related: user id=1 beserta semua post-nya
    let result: Option<WithChildren<RelUser, RelPost>> =
        find_one_with_related::<RelUser, RelPost>(&pool, 1, "user_id").await.unwrap();

    assert!(result.is_some(), "Harus menemukan user id=1");
    let data = result.unwrap();
    assert_eq!(data.parent.name, "Alice");
    assert_eq!(data.children.len(), 3, "Alice harus punya 3 post");
    println!("[test] Parent: {:?}", data.parent);
    println!("[test] Children ({}):", data.children.len());
    for p in &data.children { println!("  - {:?}", p); }

    // User yang tidak punya post
    let result_bob: Option<WithChildren<RelUser, RelPost>> =
        find_one_with_related::<RelUser, RelPost>(&pool, 2, "user_id").await.unwrap();
    let bob_data = result_bob.unwrap();
    assert_eq!(bob_data.children.len(), 0, "Bob tidak punya post");

    // User yang tidak ada
    let result_none: Option<WithChildren<RelUser, RelPost>> =
        find_one_with_related::<RelUser, RelPost>(&pool, 999, "user_id").await.unwrap();
    assert!(result_none.is_none(), "id=999 harus None");
}

// ─────────────────────────────────────────────────────────────────────────────
// Test: FIND MANY TO MANY (via Pivot Table)
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_find_many_to_many() {
    let pool = setup_relational_db("test_rel_m2m.sqlite").await;

    // Seed users & roles
    sqlx::query("INSERT INTO rel_users (id, name) VALUES (1, 'Alice'), (2, 'Bob')").execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO rel_roles (id, name) VALUES (1, 'Admin'), (2, 'Editor'), (3, 'Viewer')").execute(&pool).await.unwrap();

    // Alice → Admin + Editor; Bob → Viewer
    sqlx::query("INSERT INTO rel_user_roles (user_id, role_id) VALUES (1,1),(1,2),(2,3)").execute(&pool).await.unwrap();

    // find_many_to_many: Role milik Alice (user_id=1)
    let alice_roles = find_many_to_many::<RelRole>(
        &pool, "rel_user_roles", "role_id", "user_id", 1
    ).await.unwrap();

    assert_eq!(alice_roles.len(), 2, "Alice harus punya 2 role");
    let names: Vec<&str> = alice_roles.iter().map(|r| r.name.as_str()).collect();
    assert!(names.contains(&"Admin"));
    assert!(names.contains(&"Editor"));

    // find_many_to_many: Role milik Bob (user_id=2)
    let bob_roles = find_many_to_many::<RelRole>(
        &pool, "rel_user_roles", "role_id", "user_id", 2
    ).await.unwrap();

    assert_eq!(bob_roles.len(), 1, "Bob harus punya 1 role");
    assert_eq!(bob_roles[0].name, "Viewer");
}
