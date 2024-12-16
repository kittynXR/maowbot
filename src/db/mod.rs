use sqlx::{Pool, Sqlite, SqlitePool};
use anyhow::Result;

pub struct Database {
    pool: Pool<Sqlite>,
}

impl Database {
    pub async fn new(database_url: &str) -> anyhow::Result<Self> {
        let absolute_path = if database_url == ":memory:" {
            ":memory:".to_string()
        } else {
            let path = std::env::current_dir()?.join(database_url);
            path.to_str().unwrap().to_string()
        };

        let db_path = if absolute_path == ":memory:" {
            ":memory:".to_string()
        } else {
            // On Windows, ensure a proper file URI with forward slashes
            let mut uri_path = absolute_path.replace("\\", "/");
            // If path doesn't start with a slash, add one for a well-formed URI.
            // For example, C:/... should become file:///C:/...
            if !uri_path.starts_with('/') {
                uri_path = format!("/{}", uri_path);
            }
            format!("file://{}?mode=rwc", uri_path)
        };

        if absolute_path != ":memory:" {
            if let Some(parent) = std::path::Path::new(&absolute_path).parent() {
                if !parent.exists() {
                    std::fs::create_dir_all(parent)?;
                }
            }
        }

        println!("Connecting to SQLite database at: {}", db_path);
        let pool = SqlitePool::connect(&db_path).await?;
        println!("Connected to SQLite database!");
        Ok(Self { pool })
    }



    pub async fn migrate(&self) -> Result<()> {
        println!("Applying migrations...");
        sqlx::migrate!("./migrations").run(&self.pool).await?;
        println!("Migrations applied successfully.");
        Ok(())
    }


    pub fn pool(&self) -> &Pool<Sqlite> {
        &self.pool
    }
}