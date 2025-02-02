// tests/helpers.rs (a small test-only module)
use sqlx::{Pool, Postgres};
use sqlx::postgres::PgPoolOptions;
use crate::Database;
use crate::Error;

pub async fn create_test_db_pool() -> Result<Pool<Postgres>, Error> {
    let url = std::env::var("TEST_DATABASE_URL")
        .unwrap_or_else(|_| "postgres://maow@localhost/maowbot".to_string());

    // We can allow only 1 connection if you want to ensure a serial approach:
    // .max_connections(1)
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&url)
        .await?;

    Ok(pool)
}

/// Optionally, this wipes out everything in the DB by dropping and recreating the schema each test.
pub async fn clean_database(pool: &Pool<Postgres>) -> Result<(), Error> {
    // This will drop **all** tables in the "public" schema.
    // If you have multiple schemas, adjust accordingly.
    sqlx::query("DROP SCHEMA public CASCADE; CREATE SCHEMA public;")
        .execute(pool)
        .await?;

    Ok(())
}
