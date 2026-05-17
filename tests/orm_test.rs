use my_database_manager::{
    DatabaseConfig, Model, OrmModel, QueryFilter,
    create_table, setup_database,
    insert, update, delete, find_one, find_all, find_paginated,
};
use sqlx::{Row, any::AnyRow};

// ─────────────────────────────────────────────────────────────────────────────
// Model Struct
// ─────────────────────────────────────────────────────────────────────────────

#[allow(dead_code)]
#[derive(Debug, Clone, Model)]
#[table("products")]
struct Product {
    id: i32,
    name: String,
    price: Option<f64>,
    stock: i32,
}

impl OrmModel for Product {
    fn get_id(&self) -> i64 {
        self.id as i64
    }

    fn insert_values(&self) -> Vec<String> {
        vec![
            self.name.clone(),
            self.price.map(|p| p.to_string()).unwrap_or_default(),
            self.stock.to_string(),
        ]
    }

    fn from_row(row: AnyRow) -> Result<Self, sqlx::Error> {
        Ok(Product {
            id: row.try_get("id")?,
            name: row.try_get("name")?,
            price: row.try_get::<f64, _>("price").ok(),
            stock: row.try_get("stock")?,
        })
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Helper
// ─────────────────────────────────────────────────────────────────────────────

async fn setup_test_db(file: &str) -> sqlx::Pool<sqlx::Any> {
    let _ = std::fs::remove_file(file);
    let config = DatabaseConfig::new("sqlite", "", 0, "", "", file);
    setup_database(&config).await.unwrap();
    let pool = config.connect().await.unwrap();
    create_table::<Product>(&pool).await.unwrap();
    pool
}

// ─────────────────────────────────────────────────────────────────────────────
// Test: INSERT
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_orm_insert() {
    let pool = setup_test_db("test_orm_insert.sqlite").await;

    let product = Product { id: 0, name: "Laptop".to_string(), price: Some(1500.0), stock: 10 };
    let res = insert(&pool, &product, "sqlite").await;
    assert!(res.is_ok(), "Insert gagal: {:?}", res.err());

    // Verifikasi data ada
    let rows = sqlx::query("SELECT * FROM products")
        .fetch_all(&pool).await.unwrap();
    assert_eq!(rows.len(), 1);

    let name: String = rows[0].try_get("name").unwrap();
    assert_eq!(name, "Laptop");
}

// ─────────────────────────────────────────────────────────────────────────────
// Test: FIND ONE
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_orm_find_one() {
    let pool = setup_test_db("test_orm_find_one.sqlite").await;

    // Insert manual agar id kita tahu
    sqlx::query("INSERT INTO products (id, name, price, stock) VALUES (1, 'Monitor', 300.0, 5)")
        .execute(&pool).await.unwrap();

    let result = find_one::<Product>(&pool, 1).await.unwrap();
    assert!(result.is_some(), "find_one harus menemukan data");

    let p = result.unwrap();
    assert_eq!(p.name, "Monitor");
    assert_eq!(p.stock, 5);

    // Cari id yang tidak ada
    let none_result = find_one::<Product>(&pool, 999).await.unwrap();
    assert!(none_result.is_none(), "id 999 harus None");
}

// ─────────────────────────────────────────────────────────────────────────────
// Test: UPDATE
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_orm_update() {
    let pool = setup_test_db("test_orm_update.sqlite").await;

    sqlx::query("INSERT INTO products (id, name, price, stock) VALUES (1, 'Keyboard', 50.0, 20)")
        .execute(&pool).await.unwrap();

    // Update record
    let updated = Product { id: 1, name: "Keyboard Pro".to_string(), price: Some(75.0), stock: 15 };
    let res = update(&pool, &updated, "sqlite").await;
    assert!(res.is_ok(), "Update gagal: {:?}", res.err());

    // Verifikasi perubahan
    let p = find_one::<Product>(&pool, 1).await.unwrap().unwrap();
    assert_eq!(p.name, "Keyboard Pro");
    assert_eq!(p.stock, 15);
}

// ─────────────────────────────────────────────────────────────────────────────
// Test: DELETE
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_orm_delete() {
    let pool = setup_test_db("test_orm_delete.sqlite").await;

    sqlx::query("INSERT INTO products (id, name, price, stock) VALUES (1, 'Mouse', 25.0, 30)")
        .execute(&pool).await.unwrap();

    let res = delete::<Product>(&pool, 1).await;
    assert!(res.is_ok(), "Delete gagal: {:?}", res.err());

    let none_result = find_one::<Product>(&pool, 1).await.unwrap();
    assert!(none_result.is_none(), "Data harus sudah terhapus");
}

// ─────────────────────────────────────────────────────────────────────────────
// Test: FIND ALL
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_orm_find_all() {
    let pool = setup_test_db("test_orm_find_all.sqlite").await;

    for i in 1..=5 {
        sqlx::query("INSERT INTO products (id, name, price, stock) VALUES (?, ?, ?, ?)")
            .bind(i)
            .bind(format!("Product {}", i))
            .bind(i as f64 * 10.0)
            .bind(i * 5)
            .execute(&pool).await.unwrap();
    }

    let all = find_all::<Product>(&pool, "sqlite", None).await.unwrap();
    assert_eq!(all.len(), 5, "Harus ada 5 data");
    assert_eq!(all[0].name, "Product 1");
    assert_eq!(all[4].name, "Product 5");
}

// ─────────────────────────────────────────────────────────────────────────────
// Test: FIND PAGINATED
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_orm_find_paginated() {
    let pool = setup_test_db("test_orm_paginated.sqlite").await;

    // Insert 10 data
    for i in 1..=10 {
        sqlx::query("INSERT INTO products (id, name, price, stock) VALUES (?, ?, ?, ?)")
            .bind(i)
            .bind(format!("Item {}", i))
            .bind(i as f64 * 5.0)
            .bind(i)
            .execute(&pool).await.unwrap();
    }

    // Halaman 1: 3 data per halaman
    let page1 = find_paginated::<Product>(&pool, "sqlite", 1, 3, None).await.unwrap();
    assert_eq!(page1.data.len(), 3, "Halaman 1 harus berisi 3 data");
    assert_eq!(page1.page, 1);
    assert_eq!(page1.per_page, 3);
    assert_eq!(page1.total, 10);
    assert_eq!(page1.total_pages, 4); // ceil(10/3)
    println!("[test_orm_paginated] Page 1: {:?}", page1.data.iter().map(|p| &p.name).collect::<Vec<_>>());

    // Halaman 2
    let page2 = find_paginated::<Product>(&pool, "sqlite", 2, 3, None).await.unwrap();
    assert_eq!(page2.data.len(), 3, "Halaman 2 harus berisi 3 data");
    assert_eq!(page2.page, 2);

    // Halaman terakhir (halaman 4): hanya 1 data
    let page4 = find_paginated::<Product>(&pool, "sqlite", 4, 3, None).await.unwrap();
    assert_eq!(page4.data.len(), 1, "Halaman terakhir harus berisi 1 data");
}

// ─────────────────────────────────────────────────────────────────────────────
// Test: FILTER & SEARCH
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_orm_find_with_filter() {
    let pool = setup_test_db("test_orm_filter.sqlite").await;

    // Insert data
    sqlx::query("INSERT INTO products (id, name, price, stock) VALUES (1, 'Apple MacBook', 1500.0, 10)").execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO products (id, name, price, stock) VALUES (2, 'Apple iPhone', 1000.0, 50)").execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO products (id, name, price, stock) VALUES (3, 'Samsung Galaxy', 900.0, 30)").execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO products (id, name, price, stock) VALUES (4, 'Apple iPad', 800.0, 20)").execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO products (id, name, price, stock) VALUES (5, 'Sony Headphones', 300.0, 100)").execute(&pool).await.unwrap();

    // 1. EXACT match (stock = 10)
    let filter1 = QueryFilter::new().exact("stock", "10");
    let res1 = find_all::<Product>(&pool, "sqlite", Some(&filter1)).await.unwrap();
    assert_eq!(res1.len(), 1);
    assert_eq!(res1[0].name, "Apple MacBook");

    // 2. LIKE search (name LIKE '%Apple%') + ORDER BY price DESC
    let filter2 = QueryFilter::new()
        .like("name", "Apple")
        .order("price DESC");
    let res2 = find_all::<Product>(&pool, "sqlite", Some(&filter2)).await.unwrap();
    assert_eq!(res2.len(), 3, "Harus ada 3 produk Apple");
    assert_eq!(res2[0].name, "Apple MacBook", "Paling mahal di atas");
    assert_eq!(res2[2].name, "Apple iPad", "Paling murah di bawah");

    // 3. Kombinasi EXACT & LIKE dalam Paginated Result
    let filter3 = QueryFilter::new()
        .like("name", "a") // MacBook, Galaxy, iPad, Headphones (semua ada 'a')
        .order("id ASC");
    let page1 = find_paginated::<Product>(&pool, "sqlite", 1, 2, Some(&filter3)).await.unwrap();
    
    assert_eq!(page1.data.len(), 2, "Halaman 1 berisi 2 data");
    assert_eq!(page1.data[0].id, 1, "MacBook");
    assert_eq!(page1.data[1].id, 2, "iPhone memiliki 'a' di 'Apple'");
}
