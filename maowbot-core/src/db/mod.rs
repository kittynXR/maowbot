// maowbot-core/src/db/mod.rs
pub mod postgres_embedded;

use sqlx::postgres::{PgPoolOptions};
use sqlx::{Pool, Postgres};
use crate::Error;
use anyhow::Result;

/// Our Database struct now uses a Pool<Postgres>.
pub struct Database {
    pool: Pool<Postgres>,
}

impl Database {
    /// Create a new Database connection.
    pub async fn new(database_url: &str) -> Result<Self, Error> {
        // Example: connect to Postgres with a small pool size, just a demonstration
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect(database_url)
            .await?;

        println!("Connected to Postgres at {}", database_url);
        Ok(Self { pool })
    }

    /// Run migrations in the `migrations/` folder.
    pub async fn migrate(&self) -> Result<(), Error> {
        println!("Applying migrations...");
        sqlx::migrate!("../migrations").run(&self.pool).await?;
        println!("Migrations applied successfully.");
        Ok(())
    }

    pub fn pool(&self) -> &Pool<Postgres> {
        &self.pool
    }

    /// Optional: if you want a from_pool constructor for tests
    pub fn from_pool(pool: Pool<Postgres>) -> Self {
        Self { pool }
    }
}
