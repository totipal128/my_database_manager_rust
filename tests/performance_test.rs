use my_database_manager::{
    DatabaseConfig, Model, OrmModel, QueryFilter, create_table, find_all, find_paginated, insert,
    setup_database,
};
use sqlx::{Row, any::AnyRow};
use std::time::Instant;

// ─────────────────────────────────────────────────────────────────────────────
// Model Struct
// ─────────────────────────────────────────────────────────────────────────────

#[allow(dead_code)]
#[derive(Debug, Clone, Model)]
#[table("perf_logs")]
struct PerfLog {
    id: i32,
    level: String,
    message: String,
}

impl OrmModel for PerfLog {
    fn get_id(&self) -> i64 {
        self.id as i64
    }
    fn insert_values(&self) -> Vec<String> {
        vec![self.level.clone(), self.message.clone()]
    }
    fn from_row(row: AnyRow) -> Result<Self, sqlx::Error> {
        Ok(PerfLog {
            id: row.try_get("id")?,
            level: row.try_get("level")?,
            message: row.try_get("message")?,
        })
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Helper Setup
// ─────────────────────────────────────────────────────────────────────────────

async fn setup_perf_db(file: &str) -> sqlx::Pool<sqlx::Any> {
    let _ = std::fs::remove_file(file); // Bersihkan DB lama jika ada
    let config = DatabaseConfig::new("sqlite", "", 0, "", "", file);
    setup_database(&config).await.unwrap();
    let pool = config.connect().await.unwrap();
    create_table::<PerfLog>(&pool).await.unwrap();
    pool
}

// ─────────────────────────────────────────────────────────────────────────────
// Performance Tests
// ─────────────────────────────────────────────────────────────────────────────

// Gunakan ignore secara default agar tidak memperlambat `cargo test` biasa ,
// jalankan spesifik dengan: `cargo test --release test_orm_performance -- --ignored --nocapture`
#[tokio::test]
#[ignore]
async fn test_orm_performance() {
    let pool = setup_perf_db("test_perf.sqlite").await;
    let total_records = 10_000;

    println!("============================================================");
    println!("MEMULAI PENGUJIAN PERFORMA ({} Record)", total_records);
    println!("============================================================");

    // 1. BENCHMARK: BULK INSERT (menggunakan perulangan insert)
    let start_insert = Instant::now();
    for i in 1..=total_records {
        let level = if i % 10 == 0 {
            "ERROR"
        } else if i % 5 == 0 {
            "WARN"
        } else {
            "INFO"
        };
        let log = PerfLog {
            id: 0,
            level: level.to_string(),
            message: format!("Pesan log ke-{}", i),
        };
        // Perhatikan: Dalam aplikasi produksi yang nyata, Bulk Insert sebaiknya
        // menggunakan 1 query besar ketimbang looping insert per-baris.
        // Ini murni menguji kecepatan fungsi insert bawaan.
        insert(&pool, &log, "sqlite").await.unwrap();
    }
    let duration_insert = start_insert.elapsed();
    println!(
        "1. Waktu Insert {} baris : {:?}",
        total_records, duration_insert
    );

    // 2. BENCHMARK: FIND ALL
    let start_find_all = Instant::now();
    let all = find_all::<PerfLog>(&pool, "sqlite", None).await.unwrap();
    let duration_find_all = start_find_all.elapsed();
    assert_eq!(all.len(), total_records);
    println!(
        "2. Waktu Find All ({} baris) : {:?}",
        total_records, duration_find_all
    );

    // 3. BENCHMARK: FIND PAGINATED (Page 50, 100 baris/halaman)
    let start_paginated = Instant::now();
    let page = find_paginated::<PerfLog>(&pool, "sqlite", 50, 100, None)
        .await
        .unwrap();
    let duration_paginated = start_paginated.elapsed();
    assert_eq!(page.data.len(), 100);
    assert_eq!(page.total, total_records as u64);
    println!(
        "3. Waktu Paginasi (Page 50, 100 item): {:?}",
        duration_paginated
    );

    // 4. BENCHMARK: FILTER EXACT (Cari yang level = 'ERROR' -> 10% dari data)
    let filter_exact = QueryFilter::new().exact("level", "ERROR");
    let start_filter_exact = Instant::now();
    let errors = find_all::<PerfLog>(&pool, "sqlite", Some(&filter_exact))
        .await
        .unwrap();
    let duration_filter_exact = start_filter_exact.elapsed();
    assert_eq!(errors.len(), total_records / 10);
    println!(
        "4. Waktu Filter Exact (level='ERROR') : {:?}",
        duration_filter_exact
    );

    // 5. BENCHMARK: FILTER LIKE (Pencarian string)
    let filter_like = QueryFilter::new().like("message", "5000"); // Harusnya menemukan "Pesan log ke-5000"
    let start_filter_like = Instant::now();
    let search_res = find_all::<PerfLog>(&pool, "sqlite", Some(&filter_like))
        .await
        .unwrap();
    let duration_filter_like = start_filter_like.elapsed();
    assert!(!search_res.is_empty());
    println!(
        "5. Waktu Filter Like (message='5000') : {:?}",
        duration_filter_like
    );

    println!("============================================================");
    println!("SELESAI");
    println!("============================================================");
}
