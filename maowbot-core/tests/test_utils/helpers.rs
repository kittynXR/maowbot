// File: maowbot-core/tests/test_utils/mod.rs

use sqlx::{Pool, Postgres, Executor};
use sqlx::postgres::PgPoolOptions;
use maowbot_core::{Database, Error};

/// Create a connection pool to the test DB.
/// By default looks for `TEST_DATABASE_URL` in env,
/// else uses `postgres://maow@localhost/maowbot_test`.
pub async fn create_test_db_pool() -> Result<Pool<Postgres>, Error> {
    let url = std::env::var("TEST_DATABASE_URL")
        .unwrap_or_else(|_| "postgres://maow@localhost/maowbot_test".to_string());

    let pool = PgPoolOptions::new()
        .max_connections(5) // or 1 if you want fully serial tests
        .connect(&url)
        .await?;

    Ok(pool)
}

/// Wipes out test data so each test can start fresh.
///
/// Adjust the list of tables here to match your schema.
/// For example, you can either:
///   - Truncate all known tables, OR
///   - Drop and recreate the schema entirely, OR
///   - Use whatever approach best suits your testing style.
///
/// Below is an example “TRUNCATE ... CASCADE” pattern.
pub async fn clean_database(pool: &Pool<Postgres>) -> Result<(), Error> {
    sqlx::query(r#"
        TRUNCATE TABLE
            users,
            platform_identities,
            platform_credentials,
            user_analysis,
            user_analysis_history,
            link_requests,
            user_audit_log,
            daily_stats,
            chat_sessions,
            bot_events,
            command_logs,
            chat_messages,
            plugin_events,
            maintenance_state
        RESTART IDENTITY CASCADE;
    "#)
        .execute(pool)
        .await?;

    Ok(())
}

/// If you want a convenience function that returns a fully migrated Database:
pub async fn setup_test_database() -> Result<Database, Error> {
    let pool = create_test_db_pool().await?;
    // Wrap in our maowbot_core::Database
    let db = Database::from_pool(pool);
    // Run migrations
    db.migrate().await?;
    // Clean existing data
    clean_database(db.pool()).await?;
    Ok(db)
}