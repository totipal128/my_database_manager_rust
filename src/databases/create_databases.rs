use sqlx::{migrate::MigrateDatabase, Any};
use crate::databases::driver::DatabaseConfig;

pub async fn setup_database(config: &DatabaseConfig) -> Result<(), sqlx::Error> {
    // Memastikan driver tersedia (penting untuk Any)
    sqlx::any::install_default_drivers();

    let url = config.to_url();

    // Mengecek apakah database sudah ada
    if !Any::database_exists(&url).await.unwrap_or(false) {
        println!("Database belum ada. Membuat database: {}", config.db_name);
        
        // Membuat database
        match Any::create_database(&url).await {
            Ok(_) => println!("Berhasil membuat database: {}", config.db_name),
            Err(error) => {
                println!("Gagal membuat database: {}", error);
                return Err(error);
            }
        }
    } else {
        println!("Database {} sudah ada, melewati proses pembuatan.", config.db_name);
    }

    Ok(())
}