use my_database_manager::{
    Model, OrmModel, DbValue, QueryFilter,
    create_table, insert, update, delete,
    find_one, find_all, find_by, find_paginated, find_by_paginated, PaginatedResult,
};
use sqlx::any::AnyRow;
use sqlx::{Row, any::AnyPoolOptions};

// ═════════════════════════════════════════════════════════════════════════════
// TEST MODEL: Barang sederhana (mirip dengan model asli di aplikasi)
// ═════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Model)]
#[table("test_barang")]
struct TestBarang {
    id: i32,
    nama: String,
    harga: f64,
    stok: f64,
    is_active: bool,
    kategori: String,
    deskripsi: Option<String>,
}

impl OrmModel for TestBarang {
    fn get_id(&self) -> i64 {
        self.id as i64
    }

    fn insert_values(&self) -> Vec<DbValue> {
        vec![
            DbValue::String(self.nama.clone()),
            DbValue::Float(self.harga),
            DbValue::Float(self.stok),
            DbValue::Bool(self.is_active),
            DbValue::String(self.kategori.clone()),
            match &self.deskripsi {
                Some(d) => DbValue::String(d.clone()),
                None => DbValue::Null,
            },
        ]
    }

    fn from_row(row: AnyRow) -> Result<Self, sqlx::Error> {
        // NOTE: is_active dibaca secara universal:
        // - SQLite: BOOLEAN → INTEGER, baca sebagai i32
        // - MySQL:  BOOLEAN → TINYINT, baca sebagai bool (supported)
        // - PostgreSQL: BOOLEAN native, baca sebagai bool
        // Fallback: jika bool gagal, coba baca sebagai i32 (untuk SQLite)
        let is_active = row.try_get::<bool, _>("is_active")
            .or_else(|_| row.try_get::<i32, _>("is_active").map(|v| v != 0))
            .unwrap_or(false);
        Ok(TestBarang {
            id: row.try_get("id")?,
            nama: row.try_get("nama")?,
            harga: row.try_get("harga")?,
            stok: row.try_get("stok")?,
            is_active,
            kategori: row.try_get("kategori")?,
            deskripsi: row.try_get("deskripsi").ok(),
        })
    }
}

// ═════════════════════════════════════════════════════════════════════════════
// TEST MODEL: Parent-Child (untuk relasi)
// ═════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Model)]
#[table("test_kategori")]
struct TestKategori {
    id: i32,
    nama: String,
}

impl OrmModel for TestKategori {
    fn get_id(&self) -> i64 {
        self.id as i64
    }
    fn insert_values(&self) -> Vec<DbValue> {
        vec![DbValue::String(self.nama.clone())]
    }
    fn from_row(row: AnyRow) -> Result<Self, sqlx::Error> {
        Ok(TestKategori {
            id: row.try_get("id")?,
            nama: row.try_get("nama")?,
        })
    }
}

#[derive(Debug, Clone, Model)]
#[table("test_produk")]
struct TestProduk {
    id: i32,
    nama: String,
    id_kategori: i64,
    harga: f64,
}

impl OrmModel for TestProduk {
    fn get_id(&self) -> i64 {
        self.id as i64
    }
    fn insert_values(&self) -> Vec<DbValue> {
        vec![
            DbValue::String(self.nama.clone()),
            DbValue::Int(self.id_kategori),
            DbValue::Float(self.harga),
        ]
    }
    fn from_row(row: AnyRow) -> Result<Self, sqlx::Error> {
        Ok(TestProduk {
            id: row.try_get("id")?,
            nama: row.try_get("nama")?,
            id_kategori: row.try_get("id_kategori")?,
            harga: row.try_get("harga")?,
        })
    }
}

// ═════════════════════════════════════════════════════════════════════════════
// HELPERS
// ═════════════════════════════════════════════════════════════════════════════

async fn setup_sqlite() -> (sqlx::AnyPool, String) {
    sqlx::any::install_default_drivers();
    // NOTE: max_connections(1) penting untuk SQLite in-memory
    // karena setiap koneksi punya database terpisah
    let pool = AnyPoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("Failed to connect to SQLite in-memory");
    let driver = "sqlite".to_string();

    create_table::<TestBarang>(&pool, &driver)
        .await
        .expect("Failed to create test_barang table");
    create_table::<TestKategori>(&pool, &driver)
        .await
        .expect("Failed to create test_kategori table");
    create_table::<TestProduk>(&pool, &driver)
        .await
        .expect("Failed to create test_produk table");

    (pool, driver)
}

async fn insert_sample_barang(pool: &sqlx::AnyPool, driver: &str) {
    let items = vec![
        TestBarang {
            id: 0,
            nama: "Beras Premium".into(),
            harga: 15000.0,
            stok: 50.0,
            is_active: true,
            kategori: "Sembako".into(),
            deskripsi: Some("Beras 5kg".into()),
        },
        TestBarang {
            id: 0,
            nama: "Gula Pasir".into(),
            harga: 14000.0,
            stok: 30.0,
            is_active: true,
            kategori: "Sembako".into(),
            deskripsi: None,
        },
        TestBarang {
            id: 0,
            nama: "Minyak Goreng".into(),
            harga: 22000.0,
            stok: 20.0,
            is_active: true,
            kategori: "Sembako".into(),
            deskripsi: Some("Minyak 2L".into()),
        },
        TestBarang {
            id: 0,
            nama: "Kopi Bubuk".into(),
            harga: 25000.0,
            stok: 10.0,
            is_active: false,
            kategori: "Minuman".into(),
            deskripsi: None,
        },
        TestBarang {
            id: 0,
            nama: "Teh Celup".into(),
            harga: 8000.0,
            stok: 100.0,
            is_active: true,
            kategori: "Minuman".into(),
            deskripsi: Some("Teh 25 kantong".into()),
        },
    ];

    for item in items {
        insert(pool, &item, driver).await.unwrap();
    }
}

// ═════════════════════════════════════════════════════════════════════════════
// TESTS: CREATE TABLE (type mapping)
// ═════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_double_precision_maps_to_real_in_sqlite() {
    sqlx::any::install_default_drivers();
    let pool = AnyPoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("Connect SQLite");
    let driver = "sqlite";

    // Buat tabel dengan DOUBLE PRECISION fields
    create_table::<TestBarang>(&pool, driver)
        .await
        .expect("Create table");

    // Insert & baca back — pastikan float precision terjaga
    let item = TestBarang {
        id: 0,
        nama: "Precision Test".into(),
        harga: 12345.6789,
        stok: 0.001,
        is_active: true,
        kategori: "Test".into(),
        deskripsi: None,
    };
    insert(&pool, &item, driver).await.unwrap();

    let found = find_one::<TestBarang>(&pool, driver, 1)
        .await
        .unwrap()
        .expect("Should find inserted item");

    assert!(
        (found.harga - 12345.6789).abs() < 0.001,
        "DOUBLE PRECISION f64 value should be preserved (harga)"
    );
    assert!(
        (found.stok - 0.001).abs() < 0.0001,
        "DOUBLE PRECISION f64 value should be preserved (stok)"
    );
}

#[tokio::test]
async fn test_boolean_stored_and_read() {
    let (pool, driver) = setup_sqlite().await;

    let item_true = TestBarang {
        id: 0,
        nama: "Active".into(),
        harga: 100.0,
        stok: 10.0,
        is_active: true,
        kategori: "Test".into(),
        deskripsi: None,
    };
    let item_false = TestBarang {
        id: 0,
        nama: "Inactive".into(),
        harga: 200.0,
        stok: 20.0,
        is_active: false,
        kategori: "Test".into(),
        deskripsi: None,
    };

    insert(&pool, &item_true, &driver).await.unwrap();
    insert(&pool, &item_false, &driver).await.unwrap();

    let found1 = find_one::<TestBarang>(&pool, &driver, 1)
        .await
        .unwrap()
        .unwrap();
    let found2 = find_one::<TestBarang>(&pool, &driver, 2)
        .await
        .unwrap()
        .unwrap();

    assert!(found1.is_active, "Item 1 should be active (true)");
    assert!(!found2.is_active, "Item 2 should be inactive (false)");
}

#[tokio::test]
async fn test_nullable_field() {
    let (pool, driver) = setup_sqlite().await;

    // Insert with None deskripsi
    let item = TestBarang {
        id: 0,
        nama: "No Desc".into(),
        harga: 100.0,
        stok: 10.0,
        is_active: true,
        kategori: "Test".into(),
        deskripsi: None,
    };
    insert(&pool, &item, &driver).await.unwrap();

    let found = find_one::<TestBarang>(&pool, &driver, 1)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(found.deskripsi, None, "Nullable field should be None");

    // Insert with Some deskripsi
    let item2 = TestBarang {
        deskripsi: Some("Ada deskripsi".into()),
        ..item
    };
    insert(&pool, &item2, &driver).await.unwrap();

    let found2 = find_one::<TestBarang>(&pool, &driver, 2)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        found2.deskripsi,
        Some("Ada deskripsi".into()),
        "Nullable field should contain value"
    );
}

// ═════════════════════════════════════════════════════════════════════════════
// TESTS: INSERT & FIND_ONE
// ═════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_insert_and_find_one() {
    let (pool, driver) = setup_sqlite().await;

    let item = TestBarang {
        id: 0,
        nama: "Test Item".into(),
        harga: 100.0,
        stok: 10.0,
        is_active: true,
        kategori: "Test".into(),
        deskripsi: Some("Test deskripsi".into()),
    };
    insert(&pool, &item, &driver).await.unwrap();

    let found = find_one::<TestBarang>(&pool, &driver, 1)
        .await
        .unwrap()
        .expect("Should find inserted item");

    assert_eq!(found.nama, "Test Item");
    assert_eq!(found.harga, 100.0);
    assert_eq!(found.stok, 10.0);
    assert!(found.is_active);
    assert_eq!(found.kategori, "Test");
    assert_eq!(found.deskripsi, Some("Test deskripsi".into()));
    assert_eq!(found.id, 1, "Auto-increment id should be 1");
}

#[tokio::test]
async fn test_find_one_not_found() {
    let (pool, driver) = setup_sqlite().await;

    let found = find_one::<TestBarang>(&pool, &driver, 999)
        .await
        .unwrap();
    assert!(found.is_none(), "Non-existent id should return None");
}

// ═════════════════════════════════════════════════════════════════════════════
// TESTS: FIND_ALL
// ═════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_find_all_no_filter() {
    let (pool, driver) = setup_sqlite().await;
    insert_sample_barang(&pool, &driver).await;

    let items = find_all::<TestBarang>(&pool, &driver, None)
        .await
        .unwrap();
    assert_eq!(items.len(), 5, "Should return all 5 items");
}

#[tokio::test]
async fn test_find_all_empty_table() {
    let (pool, driver) = setup_sqlite().await;

    let items = find_all::<TestBarang>(&pool, &driver, None)
        .await
        .unwrap();
    assert_eq!(items.len(), 0, "Empty table should return 0 items");
}

// ═════════════════════════════════════════════════════════════════════════════
// TESTS: FIND_BY
// ═════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_find_by_kategori() {
    let (pool, driver) = setup_sqlite().await;
    insert_sample_barang(&pool, &driver).await;

    let sembako = find_by::<TestBarang>(&pool, &driver, "kategori", "Sembako")
        .await
        .unwrap();
    assert_eq!(sembako.len(), 3, "Should find 3 Sembako items");

    let minuman = find_by::<TestBarang>(&pool, &driver, "kategori", "Minuman")
        .await
        .unwrap();
    assert_eq!(minuman.len(), 2, "Should find 2 Minuman items");
}

#[tokio::test]
async fn test_find_by_nama() {
    let (pool, driver) = setup_sqlite().await;
    insert_sample_barang(&pool, &driver).await;

    let found = find_by::<TestBarang>(&pool, &driver, "nama", "Beras Premium")
        .await
        .unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].harga, 15000.0);
}

#[tokio::test]
async fn test_find_by_no_match() {
    let (pool, driver) = setup_sqlite().await;
    insert_sample_barang(&pool, &driver).await;

    let found = find_by::<TestBarang>(&pool, &driver, "kategori", "Nonexistent")
        .await
        .unwrap();
    assert_eq!(found.len(), 0, "No match should return empty vec");
}

// ═════════════════════════════════════════════════════════════════════════════
// TESTS: UPDATE
// ═════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_update_fields() {
    let (pool, driver) = setup_sqlite().await;

    let item = TestBarang {
        id: 0,
        nama: "Original".into(),
        harga: 100.0,
        stok: 10.0,
        is_active: true,
        kategori: "Test".into(),
        deskripsi: None,
    };
    insert(&pool, &item, &driver).await.unwrap();

    // Update item (id=1)
    let updated = TestBarang {
        id: 1,
        nama: "Updated".into(),
        harga: 200.0,
        stok: 20.0,
        is_active: false,
        kategori: "Test".into(),
        deskripsi: Some("Changed".into()),
    };
    update(&pool, &updated, &driver).await.unwrap();

    let found = find_one::<TestBarang>(&pool, &driver, 1)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(found.nama, "Updated");
    assert_eq!(found.harga, 200.0);
    assert_eq!(found.stok, 20.0);
    assert!(!found.is_active);
    assert_eq!(found.deskripsi, Some("Changed".into()));
}

#[tokio::test]
async fn test_update_partial() {
    let (pool, driver) = setup_sqlite().await;

    let item = TestBarang {
        id: 0,
        nama: "Original".into(),
        harga: 100.0,
        stok: 10.0,
        is_active: true,
        kategori: "Test".into(),
        deskripsi: Some("Original desc".into()),
    };
    insert(&pool, &item, &driver).await.unwrap();

    // Update only nama and harga
    let updated = TestBarang {
        id: 1,
        nama: "Updated Name".into(),
        harga: 999.0,
        stok: 10.0,  // unchanged
        is_active: true, // unchanged
        kategori: "Test".into(),
        deskripsi: Some("Original desc".into()), // unchanged
    };
    update(&pool, &updated, &driver).await.unwrap();

    let found = find_one::<TestBarang>(&pool, &driver, 1)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(found.nama, "Updated Name");
    assert_eq!(found.harga, 999.0);
    assert_eq!(found.stok, 10.0, "Unchanged field should stay");
    assert_eq!(
        found.deskripsi,
        Some("Original desc".into()),
        "Unchanged field should stay"
    );
}

// ═════════════════════════════════════════════════════════════════════════════
// TESTS: DELETE
// ═════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_delete_existing() {
    let (pool, driver) = setup_sqlite().await;
    insert_sample_barang(&pool, &driver).await;

    // Verify item exists
    let before = find_one::<TestBarang>(&pool, &driver, 1)
        .await
        .unwrap();
    assert!(before.is_some());

    delete::<TestBarang>(&pool, &driver, 1).await.unwrap();

    let after = find_one::<TestBarang>(&pool, &driver, 1)
        .await
        .unwrap();
    assert!(after.is_none(), "Deleted item should not exist");
}

#[tokio::test]
async fn test_delete_non_existent() {
    let (pool, driver) = setup_sqlite().await;

    // Deleting non-existent id should not error
    let result = delete::<TestBarang>(&pool, &driver, 999).await;
    assert!(result.is_ok(), "Deleting non-existent id should not error");
}

#[tokio::test]
async fn test_delete_does_not_affect_other_records() {
    let (pool, driver) = setup_sqlite().await;
    insert_sample_barang(&pool, &driver).await;

    // Delete one item
    delete::<TestBarang>(&pool, &driver, 1).await.unwrap();

    // Other items should still exist
    let remaining = find_all::<TestBarang>(&pool, &driver, None)
        .await
        .unwrap();
    assert_eq!(remaining.len(), 4, "Should have 4 items after deleting 1");
}

// ═════════════════════════════════════════════════════════════════════════════
// TESTS: QUERYFILTER (exact, like, order)
// ═════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_query_filter_exact() {
    let (pool, driver) = setup_sqlite().await;
    insert_sample_barang(&pool, &driver).await;

    let filter = QueryFilter::new().exact("kategori", "Minuman");
    let items = find_all::<TestBarang>(&pool, &driver, Some(&filter))
        .await
        .unwrap();
    assert_eq!(items.len(), 2);
    for item in &items {
        assert_eq!(item.kategori, "Minuman");
    }
}

#[tokio::test]
async fn test_query_filter_multiple_exact() {
    let (pool, driver) = setup_sqlite().await;
    insert_sample_barang(&pool, &driver).await;

    // Filter by kategori AND nama
    let filter = QueryFilter::new()
        .exact("kategori", "Sembako")
        .exact("nama", "Beras Premium");
    let items = find_all::<TestBarang>(&pool, &driver, Some(&filter))
        .await
        .unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].nama, "Beras Premium");
}

#[tokio::test]
async fn test_query_filter_like() {
    let (pool, driver) = setup_sqlite().await;
    insert_sample_barang(&pool, &driver).await;

    // Search for items with "goreng" in name (case sensitive in SQLite LIKE)
    let filter = QueryFilter::new().like("nama", "Goreng");
    let items = find_all::<TestBarang>(&pool, &driver, Some(&filter))
        .await
        .unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].nama, "Minyak Goreng");
}

#[tokio::test]
async fn test_query_filter_order_by() {
    let (pool, driver) = setup_sqlite().await;
    insert_sample_barang(&pool, &driver).await;

    let filter = QueryFilter::new().order("harga DESC");
    let items = find_all::<TestBarang>(&pool, &driver, Some(&filter))
        .await
        .unwrap();
    assert_eq!(items.len(), 5);
    // Harga descending: 25000, 22000, 15000, 14000, 8000
    assert_eq!(items[0].harga, 25000.0);
    assert_eq!(items[4].harga, 8000.0);
}

#[tokio::test]
async fn test_query_filter_combined() {
    let (pool, driver) = setup_sqlite().await;
    insert_sample_barang(&pool, &driver).await;

    // Filter + order
    let filter = QueryFilter::new()
        .exact("kategori", "Sembako")
        .order("harga ASC");
    let items = find_all::<TestBarang>(&pool, &driver, Some(&filter))
        .await
        .unwrap();
    assert_eq!(items.len(), 3);
    // Harga ASC: 14000 (Gula), 15000 (Beras), 22000 (Minyak)
    assert_eq!(items[0].nama, "Gula Pasir");
    assert_eq!(items[2].nama, "Minyak Goreng");
}

// ═════════════════════════════════════════════════════════════════════════════
// TESTS: FIND_PAGINATED
// ═════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_find_paginated_basic() {
    let (pool, driver) = setup_sqlite().await;
    insert_sample_barang(&pool, &driver).await;

    let result: PaginatedResult<TestBarang> =
        find_paginated(&pool, &driver, 1, 2, None).await.unwrap();

    assert_eq!(result.page, 1);
    assert_eq!(result.per_page, 2);
    assert_eq!(result.data.len(), 2);
    assert_eq!(result.total, 5);
    assert_eq!(result.total_pages, 3);
}

#[tokio::test]
async fn test_find_paginated_page_2() {
    let (pool, driver) = setup_sqlite().await;
    insert_sample_barang(&pool, &driver).await;

    let result: PaginatedResult<TestBarang> =
        find_paginated(&pool, &driver, 2, 2, None).await.unwrap();

    assert_eq!(result.page, 2);
    assert_eq!(result.data.len(), 2);
    assert_eq!(result.total, 5);
}

#[tokio::test]
async fn test_find_paginated_last_page() {
    let (pool, driver) = setup_sqlite().await;
    insert_sample_barang(&pool, &driver).await;

    let result: PaginatedResult<TestBarang> =
        find_paginated(&pool, &driver, 3, 2, None).await.unwrap();

    assert_eq!(result.page, 3);
    assert_eq!(result.data.len(), 1); // only 1 item on last page
    assert_eq!(result.total, 5);
}

#[tokio::test]
async fn test_find_paginated_with_filter() {
    let (pool, driver) = setup_sqlite().await;
    insert_sample_barang(&pool, &driver).await;

    let filter = QueryFilter::new().exact("kategori", "Sembako");
    let result: PaginatedResult<TestBarang> =
        find_paginated(&pool, &driver, 1, 10, Some(&filter))
            .await
            .unwrap();

    assert_eq!(result.total, 3);
    assert_eq!(result.data.len(), 3);
}

#[tokio::test]
async fn test_find_paginated_empty() {
    let (pool, driver) = setup_sqlite().await;

    let result: PaginatedResult<TestBarang> =
        find_paginated(&pool, &driver, 1, 10, None).await.unwrap();

    assert_eq!(result.total, 0);
    assert_eq!(result.data.len(), 0);
    assert_eq!(result.total_pages, 0);
}

// ═════════════════════════════════════════════════════════════════════════════
// TESTS: FULL CYCLE (CRUD)
// ═════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_full_crud_cycle() {
    let (pool, driver) = setup_sqlite().await;

    // CREATE
    let item = TestBarang {
        id: 0,
        nama: "Cycle Test".into(),
        harga: 50000.0,
        stok: 5.0,
        is_active: true,
        kategori: "Test".into(),
        deskripsi: Some("Created".into()),
    };
    insert(&pool, &item, &driver).await.unwrap();

    // READ
    let found = find_one::<TestBarang>(&pool, &driver, 1)
        .await
        .unwrap()
        .expect("Should exist after insert");
    assert_eq!(found.nama, "Cycle Test");

    // UPDATE
    let updated = TestBarang {
        id: 1,
        nama: "Cycle Updated".into(),
        harga: 75000.0,
        stok: 5.0,
        is_active: true,
        kategori: "Test".into(),
        deskripsi: Some("Updated".into()),
    };
    update(&pool, &updated, &driver).await.unwrap();

    let found = find_one::<TestBarang>(&pool, &driver, 1)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(found.nama, "Cycle Updated");
    assert_eq!(found.harga, 75000.0);
    assert_eq!(found.deskripsi, Some("Updated".into()));

    // DELETE
    delete::<TestBarang>(&pool, &driver, 1).await.unwrap();
    let found = find_one::<TestBarang>(&pool, &driver, 1)
        .await
        .unwrap();
    assert!(found.is_none(), "Item should be deleted");
}

// ═════════════════════════════════════════════════════════════════════════════
// TESTS: PARENT-CHILD (relasi)
// ═════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_parent_child_relationship() {
    let (pool, driver) = setup_sqlite().await;

    // Create parent (kategori)
    let kat1 = TestKategori {
        id: 0,
        nama: "Makanan".into(),
    };
    let kat2 = TestKategori {
        id: 0,
        nama: "Minuman".into(),
    };
    insert(&pool, &kat1, &driver).await.unwrap();
    insert(&pool, &kat2, &driver).await.unwrap();

    // Create children (produk) referencing parents
    let produk = vec![
        TestProduk {
            id: 0,
            nama: "Nasi Goreng".into(),
            id_kategori: 1,
            harga: 25000.0,
        },
        TestProduk {
            id: 0,
            nama: "Mie Goreng".into(),
            id_kategori: 1,
            harga: 20000.0,
        },
        TestProduk {
            id: 0,
            nama: "Es Teh".into(),
            id_kategori: 2,
            harga: 5000.0,
        },
    ];
    for p in produk {
        insert(&pool, &p, &driver).await.unwrap();
    }

    // Find children by foreign key using find_by
    let makanan_products =
        find_by::<TestProduk>(&pool, &driver, "id_kategori", "1")
            .await
            .unwrap();
    assert_eq!(makanan_products.len(), 2);

    let minuman_products =
        find_by::<TestProduk>(&pool, &driver, "id_kategori", "2")
            .await
            .unwrap();
    assert_eq!(minuman_products.len(), 1);
    assert_eq!(minuman_products[0].nama, "Es Teh");
}

// ═════════════════════════════════════════════════════════════════════════════
// INTEGRATION TEST: MySQL
// ═════════════════════════════════════════════════════════════════════════════
// Test koneksi MySQL dan verifikasi bahwa query ORM (termasuk typed filter
// exact_int/exact_bool) berfungsi tanpa error type mismatch.
// ═════════════════════════════════════════════════════════════════════════════

async fn setup_mysql() -> (sqlx::AnyPool, String) {
    sqlx::any::install_default_drivers();
    let pool = AnyPoolOptions::new()
        .max_connections(2)
        .connect("mysql://root:root@localhost:3306/orm_test_mysql")
        .await
        .expect("Failed to connect to MySQL at localhost:3306");
    let driver = "mysql".to_string();

    // Drop if exists, then create fresh
    let _ = sqlx::query("DROP TABLE IF EXISTS test_produk").execute(&pool).await;
    let _ = sqlx::query("DROP TABLE IF EXISTS test_kategori").execute(&pool).await;
    let _ = sqlx::query("DROP TABLE IF EXISTS test_barang").execute(&pool).await;

    create_table::<TestBarang>(&pool, &driver)
        .await
        .expect("Failed to create test_barang table in MySQL");
    create_table::<TestKategori>(&pool, &driver)
        .await
        .expect("Failed to create test_kategori table in MySQL");
    create_table::<TestProduk>(&pool, &driver)
        .await
        .expect("Failed to create test_produk table in MySQL");

    (pool, driver)
}

#[tokio::test]
async fn test_mysql_connection_and_basic_crud() {
    let (pool, driver) = setup_mysql().await;

    // CREATE
    let item = TestBarang {
        id: 0,
        nama: "MySQL Test Item".into(),
        harga: 25000.0,
        stok: 100.0,
        is_active: true,
        kategori: "Test MySQL".into(),
        deskripsi: Some("Testing MySQL ORM".into()),
    };
    insert(&pool, &item, &driver).await.unwrap();

    // READ (find_one)
    let found = find_one::<TestBarang>(&pool, &driver, 1)
        .await
        .unwrap()
        .expect("Should find inserted item in MySQL");
    assert_eq!(found.nama, "MySQL Test Item");
    assert!(found.is_active);

    // READ (find_all)
    let all = find_all::<TestBarang>(&pool, &driver, None)
        .await
        .unwrap();
    assert_eq!(all.len(), 1);

    // UPDATE
    let updated = TestBarang {
        id: 1,
        nama: "MySQL Updated".into(),
        harga: 30000.0,
        stok: 50.0,
        is_active: false,
        kategori: "Test MySQL".into(),
        deskripsi: Some("Updated in MySQL".into()),
    };
    update(&pool, &updated, &driver).await.unwrap();

    let found = find_one::<TestBarang>(&pool, &driver, 1)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(found.nama, "MySQL Updated");
    assert!(!found.is_active);

    // DELETE
    delete::<TestBarang>(&pool, &driver, 1).await.unwrap();
    let found = find_one::<TestBarang>(&pool, &driver, 1)
        .await
        .unwrap();
    assert!(found.is_none());
}

#[tokio::test]
async fn test_mysql_typed_filter_exact_int() {
    let (pool, driver) = setup_mysql().await;

    let items = vec![
        TestBarang {
            id: 0, nama: "Item A".into(), harga: 100.0, stok: 10.0,
            is_active: true, kategori: "A".into(), deskripsi: None,
        },
        TestBarang {
            id: 0, nama: "Item B".into(), harga: 200.0, stok: 20.0,
            is_active: true, kategori: "B".into(), deskripsi: None,
        },
    ];
    for item in items {
        insert(&pool, &item, &driver).await.unwrap();
    }

    // Test exact_int filter pada id (integer column)
    let filter = QueryFilter::new().exact_int("id", 1);
    let found = find_all::<TestBarang>(&pool, &driver, Some(&filter))
        .await
        .unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].nama, "Item A");
}

#[tokio::test]
async fn test_mysql_typed_filter_exact_bool() {
    let (pool, driver) = setup_mysql().await;

    let items = vec![
        TestBarang {
            id: 0, nama: "Active Item".into(), harga: 100.0, stok: 10.0,
            is_active: true, kategori: "Test".into(), deskripsi: None,
        },
        TestBarang {
            id: 0, nama: "Inactive Item".into(), harga: 200.0, stok: 20.0,
            is_active: false, kategori: "Test".into(), deskripsi: None,
        },
    ];
    for item in items {
        insert(&pool, &item, &driver).await.unwrap();
    }

    // Test exact_bool filter pada is_active (boolean column)
    let filter = QueryFilter::new().exact_bool("is_active", true);
    let active = find_all::<TestBarang>(&pool, &driver, Some(&filter))
        .await
        .unwrap();
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].nama, "Active Item");

    let filter = QueryFilter::new().exact_bool("is_active", false);
    let inactive = find_all::<TestBarang>(&pool, &driver, Some(&filter))
        .await
        .unwrap();
    assert_eq!(inactive.len(), 1);
    assert_eq!(inactive[0].nama, "Inactive Item");
}

#[tokio::test]
async fn test_mysql_find_by_with_integer_column() {
    let (pool, driver) = setup_mysql().await;

    let items = vec![
        TestBarang {
            id: 0, nama: "Item X".into(), harga: 150.0, stok: 5.0,
            is_active: true, kategori: "Test".into(), deskripsi: None,
        },
        TestBarang {
            id: 0, nama: "Item Y".into(), harga: 250.0, stok: 15.0,
            is_active: true, kategori: "Test".into(), deskripsi: None,
        },
    ];
    for item in items {
        insert(&pool, &item, &driver).await.unwrap();
    }

    // find_by dengan value string untuk integer column — harus tetap work
    // karena MySQL auto-convert, dan PostgreSQL pakai ::TEXT cast
    let found = find_by::<TestBarang>(&pool, &driver, "id", "2")
        .await
        .unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].nama, "Item Y");
}

#[tokio::test]
async fn test_mysql_find_paginated() {
    let (pool, driver) = setup_mysql().await;

    let kategori_list = ["A", "B", "A", "B", "A"];
    for (i, kat) in kategori_list.iter().enumerate() {
        let item = TestBarang {
            id: 0,
            nama: format!("Item {}", i + 1),
            harga: (i as f64 + 1.0) * 1000.0,
            stok: (i as f64 + 1.0) * 10.0,
            is_active: true,
            kategori: kat.to_string(),
            deskripsi: None,
        };
        insert(&pool, &item, &driver).await.unwrap();
    }

    // Test find_paginated dengan filter
    let filter = QueryFilter::new().exact("kategori", "A").order("id ASC");
    let result = find_paginated::<TestBarang>(&pool, &driver, 1, 2, Some(&filter))
        .await
        .unwrap();

    assert_eq!(result.total, 3);  // 3 items with kategori A
    assert_eq!(result.data.len(), 2);  // page 1, per_page 2
    assert_eq!(result.total_pages, 2);

    // Page 2
    let result2 = find_paginated::<TestBarang>(&pool, &driver, 2, 2, Some(&filter))
        .await
        .unwrap();
    assert_eq!(result2.data.len(), 1);
}

// ═════════════════════════════════════════════════════════════════════════════
// INTEGRATION TEST: PostgreSQL
// ═════════════════════════════════════════════════════════════════════════════
// Test koneksi PostgreSQL dan verifikasi bahwa:
// 1. exact_int → menggunakan CAST($1 AS INTEGER)
// 2. exact_bool → menggunakan CAST($1 AS BOOLEAN)
// 3. find_by → menggunakan column::TEXT untuk integer column lookup
// ═════════════════════════════════════════════════════════════════════════════

async fn setup_pg() -> (sqlx::AnyPool, String) {
    sqlx::any::install_default_drivers();
    // Koneksi ke PostgreSQL container yang berjalan di localhost:5432
    let pool = AnyPoolOptions::new()
        .max_connections(2)
        .connect("postgres://user:password@localhost:5432/orm_test_pg")
        .await
        .expect("Failed to connect to PostgreSQL at localhost:5432");
    let driver = "postgres".to_string();

    // Drop if exists, then create fresh
    let _ = sqlx::query("DROP TABLE IF EXISTS test_produk CASCADE").execute(&pool).await;
    let _ = sqlx::query("DROP TABLE IF EXISTS test_kategori CASCADE").execute(&pool).await;
    let _ = sqlx::query("DROP TABLE IF EXISTS test_barang CASCADE").execute(&pool).await;

    create_table::<TestBarang>(&pool, &driver)
        .await
        .expect("Failed to create test_barang table in PostgreSQL");
    create_table::<TestKategori>(&pool, &driver)
        .await
        .expect("Failed to create test_kategori table in PostgreSQL");
    create_table::<TestProduk>(&pool, &driver)
        .await
        .expect("Failed to create test_produk table in PostgreSQL");

    (pool, driver)
}

#[tokio::test]
async fn test_pg_connection_and_basic_crud() {
    let (pool, driver) = setup_pg().await;

    // CREATE
    let item = TestBarang {
        id: 0,
        nama: "PG Test Item".into(),
        harga: 50000.0,
        stok: 200.0,
        is_active: true,
        kategori: "Test PG".into(),
        deskripsi: Some("Testing PostgreSQL ORM".into()),
    };
    insert(&pool, &item, &driver).await.unwrap();

    // READ (find_one)
    let found = find_one::<TestBarang>(&pool, &driver, 1)
        .await
        .unwrap()
        .expect("Should find inserted item in PostgreSQL");
    assert_eq!(found.nama, "PG Test Item");
    assert!(found.is_active);
    assert_eq!(found.kategori, "Test PG");

    // READ (find_all)
    let all = find_all::<TestBarang>(&pool, &driver, None)
        .await
        .unwrap();
    assert_eq!(all.len(), 1);

    // UPDATE
    let updated = TestBarang {
        id: 1,
        nama: "PG Updated".into(),
        harga: 75000.0,
        stok: 100.0,
        is_active: false,
        kategori: "Test PG".into(),
        deskripsi: Some("Updated in PostgreSQL".into()),
    };
    update(&pool, &updated, &driver).await.unwrap();

    let found = find_one::<TestBarang>(&pool, &driver, 1)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(found.nama, "PG Updated");
    assert_eq!(found.harga, 75000.0);
    assert!(!found.is_active);

    // DELETE
    delete::<TestBarang>(&pool, &driver, 1).await.unwrap();
    let found = find_one::<TestBarang>(&pool, &driver, 1)
        .await
        .unwrap();
    assert!(found.is_none());
}

#[tokio::test]
async fn test_pg_typed_filter_exact_int() {
    // Test CAST($1 AS INTEGER) untuk filter integer column
    let (pool, driver) = setup_pg().await;

    let items = vec![
        TestBarang {
            id: 0, nama: "Alpha".into(), harga: 100.0, stok: 10.0,
            is_active: true, kategori: "X".into(), deskripsi: None,
        },
        TestBarang {
            id: 0, nama: "Beta".into(), harga: 200.0, stok: 20.0,
            is_active: true, kategori: "Y".into(), deskripsi: None,
        },
    ];
    for item in items {
        insert(&pool, &item, &driver).await.unwrap();
    }

    // Filter exact_int pada id — PostgreSQL harus menggunakan CAST($1 AS INTEGER)
    let filter = QueryFilter::new().exact_int("id", 1);
    let found = find_all::<TestBarang>(&pool, &driver, Some(&filter))
        .await
        .unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].nama, "Alpha");

    // Filter exact_int untuk id yang tidak ada
    let filter = QueryFilter::new().exact_int("id", 999);
    let found = find_all::<TestBarang>(&pool, &driver, Some(&filter))
        .await
        .unwrap();
    assert_eq!(found.len(), 0);
}

#[tokio::test]
async fn test_pg_typed_filter_exact_bool() {
    // Test CAST($1 AS BOOLEAN) untuk filter boolean column
    let (pool, driver) = setup_pg().await;

    let items = vec![
        TestBarang {
            id: 0, nama: "Active".into(), harga: 100.0, stok: 10.0,
            is_active: true, kategori: "Test".into(), deskripsi: None,
        },
        TestBarang {
            id: 0, nama: "Inactive".into(), harga: 200.0, stok: 20.0,
            is_active: false, kategori: "Test".into(), deskripsi: None,
        },
    ];
    for item in items {
        insert(&pool, &item, &driver).await.unwrap();
    }

    // Filter exact_bool true
    let filter = QueryFilter::new().exact_bool("is_active", true);
    let active = find_all::<TestBarang>(&pool, &driver, Some(&filter))
        .await
        .unwrap();
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].nama, "Active");
    assert!(active[0].is_active);

    // Filter exact_bool false
    let filter = QueryFilter::new().exact_bool("is_active", false);
    let inactive = find_all::<TestBarang>(&pool, &driver, Some(&filter))
        .await
        .unwrap();
    assert_eq!(inactive.len(), 1);
    assert_eq!(inactive[0].nama, "Inactive");
    assert!(!inactive[0].is_active);
}

#[tokio::test]
async fn test_pg_find_by_with_integer_column() {
    // Test column::TEXT cast untuk find_by dengan string value pada integer column
    let (pool, driver) = setup_pg().await;

    let items = vec![
        TestBarang {
            id: 0, nama: "Item M".into(), harga: 150.0, stok: 5.0,
            is_active: true, kategori: "Test".into(), deskripsi: None,
        },
        TestBarang {
            id: 0, nama: "Item N".into(), harga: 250.0, stok: 15.0,
            is_active: true, kategori: "Test".into(), deskripsi: None,
        },
    ];
    for item in items {
        insert(&pool, &item, &driver).await.unwrap();
    }

    // find_by dengan string value pada integer column
    // Harusnya work karena ORM pakai column::TEXT cast untuk PostgreSQL
    let found = find_by::<TestBarang>(&pool, &driver, "id", "2")
        .await
        .unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].nama, "Item N");

    // find_by dengan string value yang tidak ada
    let found = find_by::<TestBarang>(&pool, &driver, "id", "999")
        .await
        .unwrap();
    assert_eq!(found.len(), 0);
}

#[tokio::test]
async fn test_pg_combined_filters_exact_int_and_text() {
    // Test kombinasi exact_int + exact dalam satu query
    let (pool, driver) = setup_pg().await;

    let items = vec![
        TestBarang {
            id: 0, nama: "Roti Tawar".into(), harga: 15000.0, stok: 30.0,
            is_active: true, kategori: "Makanan".into(), deskripsi: None,
        },
        TestBarang {
            id: 0, nama: "Roti Manis".into(), harga: 12000.0, stok: 25.0,
            is_active: true, kategori: "Makanan".into(), deskripsi: None,
        },
        TestBarang {
            id: 0, nama: "Air Mineral".into(), harga: 5000.0, stok: 100.0,
            is_active: true, kategori: "Minuman".into(), deskripsi: None,
        },
    ];
    for item in items {
        insert(&pool, &item, &driver).await.unwrap();
    }

    // Filter kombinasi: exact_int(id) + exact(kategori)
    let filter = QueryFilter::new()
        .exact_int("id", 1)
        .exact("kategori", "Makanan");
    let found = find_all::<TestBarang>(&pool, &driver, Some(&filter))
        .await
        .unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].nama, "Roti Tawar");
}

#[tokio::test]
async fn test_pg_find_paginated_with_typed_filter() {
    // Test pagination dengan filter typed (menggunakan CAST)
    let (pool, driver) = setup_pg().await;

    let kategori_list = ["A", "B", "A", "B", "A"];
    for (i, kat) in kategori_list.iter().enumerate() {
        let is_active_val = i % 2 == 0;  // true, false, true, false, true
        let item = TestBarang {
            id: 0,
            nama: format!("PG Item {}", i + 1),
            harga: (i as f64 + 1.0) * 1000.0,
            stok: (i as f64 + 1.0) * 10.0,
            is_active: is_active_val,
            kategori: kat.to_string(),
            deskripsi: None,
        };
        insert(&pool, &item, &driver).await.unwrap();
    }

    // Test pagination dengan exact filter pada text column
    let filter = QueryFilter::new().exact("kategori", "A").order("id ASC");
    let result = find_paginated::<TestBarang>(&pool, &driver, 1, 2, Some(&filter))
        .await
        .unwrap();
    assert_eq!(result.total, 3, "Total 3 items with kategori A");
    assert_eq!(result.data.len(), 2, "Page 1 with 2 per page");
    assert_eq!(result.total_pages, 2);

    // Page 2
    let result2 = find_paginated::<TestBarang>(&pool, &driver, 2, 2, Some(&filter))
        .await
        .unwrap();
    assert_eq!(result2.data.len(), 1);

    // Test exact_bool filter
    let filter_active = QueryFilter::new().exact_bool("is_active", true);
    let active_result = find_all::<TestBarang>(&pool, &driver, Some(&filter_active))
        .await
        .unwrap();
    assert_eq!(active_result.len(), 3);  // items with indexes 0, 2, 4

    let filter_inactive = QueryFilter::new().exact_bool("is_active", false);
    let inactive_result = find_all::<TestBarang>(&pool, &driver, Some(&filter_inactive))
        .await
        .unwrap();
    assert_eq!(inactive_result.len(), 2);  // items with indexes 1, 3
}

#[tokio::test]
async fn test_pg_find_by_paginated_with_text_column() {
    // Test find_by_paginated dengan text column — basic pagination pada text filter
    let (pool, driver) = setup_pg().await;

    let kategori_list = ["X", "Y", "X", "Y", "X"];
    for (i, kat) in kategori_list.iter().enumerate() {
        let item = TestBarang {
            id: 0,
            nama: format!("Item PG {}", i + 1),
            harga: (i as f64 + 1.0) * 100.0,
            stok: (i as f64 + 1.0) * 5.0,
            is_active: true,
            kategori: kat.to_string(),
            deskripsi: None,
        };
        insert(&pool, &item, &driver).await.unwrap();
    }

    // find_by_paginated dengan text column (kategori)
    let result = find_by_paginated::<TestBarang>(&pool, &driver, "kategori", "X", 1, 2)
        .await
        .unwrap();
    assert_eq!(result.total, 3);
    assert_eq!(result.data.len(), 2);
    assert_eq!(result.total_pages, 2);
}

#[tokio::test]
async fn test_pg_find_by_paginated_with_integer_column() {
    // Test find_by_paginated dengan string value pada INTEGER column
    // PostgreSQL harus pakai column::TEXT cast supaya WHERE id::TEXT = '1' work
    let (pool, driver) = setup_pg().await;

    for i in 0..5 {
        let item = TestBarang {
            id: 0,
            nama: format!("Item PG {}", i + 1),
            harga: (i as f64 + 1.0) * 100.0,
            stok: (i as f64 + 1.0) * 5.0,
            is_active: true,
            kategori: "Test".to_string(),
            deskripsi: None,
        };
        insert(&pool, &item, &driver).await.unwrap();
    }

    // find_by_paginated dengan string value pada integer column (id)
    // Ini akan menghasilkan WHERE id::TEXT = '1' di PostgreSQL
    let result = find_by_paginated::<TestBarang>(&pool, &driver, "id", "1", 1, 2)
        .await
        .unwrap();
    assert_eq!(result.total, 1, "Should find 1 item with id=1");
    assert_eq!(result.data.len(), 1);
    assert_eq!(result.data[0].nama, "Item PG 1");
}

// ═════════════════════════════════════════════════════════════════════════════
// TESTS: FIND_BY_PAGINATED
// ═════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_find_by_paginated_basic() {
    let (pool, driver) = setup_sqlite().await;
    insert_sample_barang(&pool, &driver).await;

    // Cari by kategori dengan pagination
    let result: PaginatedResult<TestBarang> =
        find_by_paginated(&pool, &driver, "kategori", "Sembako", 1, 2)
            .await
            .unwrap();

    assert_eq!(result.total, 3, "Total Sembako items = 3");
    assert_eq!(result.data.len(), 2, "Page 1 with 2 per page");
    assert_eq!(result.page, 1);
    assert_eq!(result.total_pages, 2);
}

#[tokio::test]
async fn test_find_by_paginated_page_2() {
    let (pool, driver) = setup_sqlite().await;
    insert_sample_barang(&pool, &driver).await;

    // Page 2 dari Sembako
    let result: PaginatedResult<TestBarang> =
        find_by_paginated(&pool, &driver, "kategori", "Sembako", 2, 2)
            .await
            .unwrap();

    assert_eq!(result.total, 3);
    assert_eq!(result.data.len(), 1, "Page 2 should have 1 remaining item");
    assert_eq!(result.page, 2);
}

#[tokio::test]
async fn test_find_by_paginated_no_match() {
    let (pool, driver) = setup_sqlite().await;
    insert_sample_barang(&pool, &driver).await;

    let result: PaginatedResult<TestBarang> =
        find_by_paginated(&pool, &driver, "kategori", "Nonexistent", 1, 10)
            .await
            .unwrap();

    assert_eq!(result.total, 0);
    assert_eq!(result.data.len(), 0);
    assert_eq!(result.total_pages, 0);
}

#[tokio::test]
async fn test_find_by_paginated_all_in_one_page() {
    let (pool, driver) = setup_sqlite().await;
    insert_sample_barang(&pool, &driver).await;

    let result: PaginatedResult<TestBarang> =
        find_by_paginated(&pool, &driver, "kategori", "Sembako", 1, 10)
            .await
            .unwrap();

    assert_eq!(result.total, 3);
    assert_eq!(result.data.len(), 3);
    assert_eq!(result.total_pages, 1);
}
