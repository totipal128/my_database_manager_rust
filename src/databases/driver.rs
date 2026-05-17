use sqlx::any::{AnyPoolOptions, install_default_drivers};
use sqlx::AnyPool;

#[derive(Debug, Clone)]
pub struct DatabaseConfig {
    pub db_name: String,
    pub db_pass: String,
    pub db_user: String,
    pub db_host: String,
    pub db_port: u16,
    pub driver: String, // postgres, mysql, sqlite, mssql
}

impl DatabaseConfig {
    pub fn new(driver: &str, host: &str, port: u16, user: &str, pass: &str, name: &str) -> Self {
        Self {
            driver: driver.to_string(),
            db_host: host.to_string(),
            db_port: port,
            db_user: user.to_string(),
            db_pass: pass.to_string(),
            db_name: name.to_string(),
        }
    }

    pub fn to_url(&self) -> String {
        match self.driver.as_str() {
            "sqlite" => {
                if self.db_name == ":memory:" {
                    "sqlite::memory:".to_string()
                } else {
                    format!("sqlite://{}", self.db_name)
                }
            },
            _ => format!(
                "{}://{}:{}@{}:{}/{}",
                self.driver, self.db_user, self.db_pass, self.db_host, self.db_port, self.db_name
            ),
        }
    }

    pub async fn connect(&self) -> Result<AnyPool, sqlx::Error> {
        install_default_drivers();

        let url = self.to_url();
        AnyPoolOptions::new().max_connections(5).connect(&url).await
    }
}
