use sqlx::pool::Pool;
use sqlx::{Any, Row};

pub trait Model: Sized + Clone + std::fmt::Debug {
    const TABLE: &'static str;
    const FIELDS_INSERT: &'static [&'static str];
    const FIELDS_DECLARATION: &'static [&'static str] = &[];
    const FOREIGN_KEYS: &'static [&'static str] = &[];
}

pub async fn create_table<T: Model>(pool: &Pool<Any>) -> Result<(), sqlx::Error> {
    let table_name = T::TABLE;
    let field_declarations = T::FIELDS_DECLARATION.join(",\n            ");
    let foreign_keys = T::FOREIGN_KEYS.join(",\n            ");
    
    let mut columns_sql = if field_declarations.is_empty() {
        "id INT PRIMARY KEY".to_string()
    } else {
        field_declarations
    };

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
                if let Ok(name) = row.try_get::<String, _>("name") {
                    existing_columns.push(name.to_lowercase());
                }
            }
        }
    } else {
        // Postgres & MySQL menggunakan information_schema
        let query = format!(
            "SELECT column_name FROM information_schema.columns WHERE table_name = '{}' OR table_name = '{}'", 
            table_name, table_name.to_lowercase()
        );
        if let Ok(rows) = sqlx::query(&query).fetch_all(pool).await {
            for row in rows {
                if let Ok(name) = row.try_get::<String, _>("column_name") {
                    existing_columns.push(name.to_lowercase());
                }
            }
        }
    }

    // Jika tabel belum ada, buat baru
    if existing_columns.is_empty() {
        println!("Tabel '{}' tidak ditemukan, membuat baru...", table_name);
        return create_table::<T>(pool).await;
    }

    // 3. Tambahkan kolom yang ada di Struct tapi belum ada di Database
    for decl in T::FIELDS_DECLARATION {
        if let Some(col_name) = decl.split_whitespace().next() {
            let clean_name = col_name.replace("`", "").replace("\"", "").to_lowercase();
            if !existing_columns.contains(&clean_name) {
                let alter_query = format!("ALTER TABLE {} ADD COLUMN {}", table_name, decl);
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
