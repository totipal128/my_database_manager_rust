use sqlx::pool::Pool;
use sqlx::{Any, Row};

pub trait Model: Sized + Clone + std::fmt::Debug {
    const TABLE: &'static str;
    const FIELDS_INSERT: &'static [&'static str];
    const FIELDS_DECLARATION: &'static [&'static str] = &[];
    const FOREIGN_KEYS: &'static [&'static str] = &[];
}

pub async fn create_table<T: Model>(pool: &Pool<Any>, driver: &str) -> Result<(), sqlx::Error> {
    let table_name = T::TABLE;
    let field_declarations = T::FIELDS_DECLARATION.join(",\n            ");
    let foreign_keys = T::FOREIGN_KEYS.join(",\n            ");
    
    let mut columns_sql = if field_declarations.is_empty() {
        "id INT PRIMARY KEY".to_string()
    } else {
        field_declarations
    };

    // Auto-increment replacement based on driver
    let clean_driver = driver.to_lowercase();
    if clean_driver == "sqlite" {
        columns_sql = columns_sql.replace("id INT PRIMARY KEY", "id INTEGER PRIMARY KEY AUTOINCREMENT");
        columns_sql = columns_sql.replace("DOUBLE PRECISION", "REAL");
        // SQLite tidak punya tipe BOOLEAN native, INTEGER affinity
        columns_sql = columns_sql.replace("BOOLEAN", "INTEGER");
    } else if clean_driver == "mysql" || clean_driver == "mariadb" {
        columns_sql = columns_sql.replace("id INT PRIMARY KEY", "id INT AUTO_INCREMENT PRIMARY KEY");
        // sqlx::Any driver tidak mendukung MySQL TINYINT (tipe BOOLEAN di MySQL)
        columns_sql = columns_sql.replace("BOOLEAN", "INT");
    } else if clean_driver == "postgres" || clean_driver == "postgresql" {
        columns_sql = columns_sql.replace("id INT PRIMARY KEY", "id SERIAL PRIMARY KEY");
        columns_sql = columns_sql.replace("DATETIME", "TIMESTAMP");
    }

    if !foreign_keys.is_empty() {
        columns_sql.push_str(",\n            ");
        columns_sql.push_str(&foreign_keys);
    }

    let sql = format!(
        "CREATE TABLE IF NOT EXISTS {} (\n            {}\n        )",
        table_name, columns_sql
    );

    println!("Running SQL:\n{}", sql);
    sqlx::query(&sql).execute(pool).await?;

    Ok(())
}

/// Fungsi untuk menyesuaikan tabel secara otomatis (menambah kolom baru atau menghapus kolom yang hilang)
pub async fn sync_table<T: Model>(pool: &Pool<Any>, driver: &str) -> Result<(), sqlx::Error> {
    let table_name = T::TABLE;
    
    // 1. Dapatkan kolom yang didefinisikan dalam Struct
    let mut defined_columns = if T::FIELDS_DECLARATION.is_empty() {
        vec!["id".to_string()]
    } else {
        Vec::new()
    };
    for decl in T::FIELDS_DECLARATION {
        if let Some(col_name) = decl.split_whitespace().next() {
            // Membersihkan nama kolom jika menggunakan backticks atau quotes
            let clean_name = col_name.replace("`", "").replace("\"", "");
            defined_columns.push(clean_name.to_lowercase());
        }
    }

    // 2. Dapatkan kolom yang ada di database fisik saat ini
    let mut existing_columns = Vec::new();
    if driver == "sqlite" {
        let query = format!("PRAGMA table_info('{}')", table_name);
        if let Ok(rows) = sqlx::query(&query).fetch_all(pool).await {
            for row in rows {
                // PRAGMA table_info: kolom index 1 = "name"
                if let Ok(name) = row.try_get::<String, _>("name") {
                    existing_columns.push(name.to_lowercase());
                }
            }
        }
    } else {
        // Postgres & MySQL menggunakan information_schema dengan filter schema aktif
        let schema_filter = if driver == "mysql" || driver == "mariadb" {
            "AND table_schema = DATABASE()"
        } else if driver == "postgres" || driver == "postgresql" {
            "AND table_schema = current_schema()"
        } else {
            ""
        };
        // Cast column_name::TEXT for PostgreSQL compatibility (column_name is type `Name`
        // which sqlx::Any driver can't decode)
        let col_expr = if driver == "postgres" || driver == "postgresql" {
            "column_name::TEXT AS col_name"
        } else {
            "column_name AS col_name"
        };
        let query = format!(
            "SELECT {} FROM information_schema.columns WHERE (table_name = '{}' OR table_name = '{}') {}", 
            col_expr, table_name, table_name.to_lowercase(), schema_filter
        );
        match sqlx::query(&query).fetch_all(pool).await {
            Ok(rows) => {
                for row in rows {
                    // Gunakan index kolom 0 agar tidak tergantung nama kolom
                    // (MySQL mengembalikan COLUMN_NAME uppercase, sqlx Any mungkin case-sensitive)
                    if let Ok(name) = row.try_get::<String, usize>(0) {
                        existing_columns.push(name.to_lowercase());
                    }
                }
            }
            Err(e) => {
                println!("[sync_table] Warning: Gagal query columns untuk tabel '{}': {}", table_name, e);
            }
        }
    }

    // Jika tabel belum ada (atau info_schema tidak menemukan kolom), buat baru
    if existing_columns.is_empty() {
        println!("Tabel '{}' tidak ditemukan, membuat baru...", table_name);
        // DROP dulu untuk memastikan tidak ada tabel "hantu" tanpa AUTO_INCREMENT
        // dari sesi sebelumnya yang menyebabkan CREATE TABLE IF NOT EXISTS menjadi no-op
        let drop_sql = if driver == "postgres" || driver == "postgresql" {
            format!("DROP TABLE IF EXISTS {} CASCADE", table_name)
        } else {
            format!("DROP TABLE IF EXISTS {}", table_name)
        };
        let _ = sqlx::query(&drop_sql).execute(pool).await;
        return create_table::<T>(pool, driver).await;
    }

    // 3. Tambahkan kolom yang ada di Struct tapi belum ada di Database
    for decl in T::FIELDS_DECLARATION {
        if let Some(col_name) = decl.split_whitespace().next() {
            let clean_name = col_name.replace("`", "").replace("\"", "").to_lowercase();
            if !existing_columns.contains(&clean_name) {
                // Hapus NOT NULL untuk ALTER TABLE karena menambah kolom NOT NULL
                // ke tabel yang sudah ada datanya akan gagal di semua engine (SQLite, PG, MySQL)
                let mutable_decl = decl
                    .replace(" NOT NULL", "")
                    .replace("NOT NULL", "");
                let alter_query = format!("ALTER TABLE {} ADD COLUMN {}", table_name, mutable_decl);
                println!("Menambah kolom baru: {}", alter_query);
                if let Err(e) = sqlx::query(&alter_query).execute(pool).await {
                    println!("Warning: Gagal menambah kolom '{}': {}", clean_name, e);
                }
            }
        }
    }

    // 4. Hapus kolom yang ada di Database tapi sudah tidak ada di Struct
    for col in existing_columns {
        if !defined_columns.contains(&col) {
            let alter_query = format!("ALTER TABLE {} DROP COLUMN {}", table_name, col);
            println!("Menghapus kolom: {}", alter_query);
            if let Err(e) = sqlx::query(&alter_query).execute(pool).await {
                println!("Warning: Gagal menghapus kolom '{}' (Mungkin driver tidak mensupport DROP COLUMN): {}", col, e);
            }
        }
    }

    Ok(())
}
