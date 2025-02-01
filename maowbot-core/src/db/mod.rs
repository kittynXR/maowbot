use sqlx::sqlite::{SqlitePoolOptions, SqliteConnectOptions};
use sqlx::{Pool, Sqlite, SqlitePool};
use anyhow::Result;
use crate::Error;

pub struct Database {
    pool: Pool<Sqlite>,
}

impl Database {
    pub async fn new(database_url: &str) -> Result<Self, Error> {
        if database_url == ":memory:" {
            // Use an in-memory DB
            // `SqliteConnectOptions` has a special method for in-memory, or you can do:
            let pool = SqlitePool::connect(":memory:").await?;
            return Ok(Self { pool });
        } else {
            // 1) Build absolute path so we can pass it to .filename(...)
            let path = std::env::current_dir()?.join(database_url);

            // If you need to log path after the `.filename(...)`, clone it:
            let path_clone = path.clone();

            // Build connect opts
            let connect_opts = SqliteConnectOptions::new()
                .filename(path) // consumes the original
                .create_if_missing(true);

            let pool = SqlitePoolOptions::new()
                .connect_with(connect_opts)
                .await?;

            println!("Connected to SQLite local file at {:?}", path_clone);

            Ok(Self { pool })

        }
    }

    pub async fn migrate(&self) -> Result<(), Error> {
        println!("Applying migrations...");
        sqlx::migrate!("../migrations").run(&self.pool).await?;
        println!("Migrations applied successfully.");
        Ok(())
    }

    pub fn pool(&self) -> &Pool<Sqlite> {
        &self.pool
    }

    /// Optional: if you want a “from_pool” constructor for tests
    pub fn from_pool(pool: SqlitePool) -> Self {
        Self { pool }
    }
}
