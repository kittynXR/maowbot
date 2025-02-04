// File: maowbot-core/src/test_utils/helpers.rs

use sqlx::{Pool, Postgres, Executor, PgConnection, Connection};
use sqlx::postgres::PgPoolOptions;
use crate::Error;
use crate::db::Database;

/// Create the test database if it does not exist yet.
pub async fn ensure_test_database_exists() -> Result<(), Error> {
    // Connect to the "postgres" database as an admin or superuser.
    // Adjust username/host as needed for your environment:
    let admin_url = std::env::var("DATABASE_ADMIN_URL")
        .unwrap_or_else(|_| "postgres://maow@localhost/postgres".to_string());

    // Attempt to connect to the "postgres" DB
    let mut conn = PgConnection::connect(&admin_url).await?;

    // The database we want to ensure is present:
    let test_db = "maowbot_test";

    // Some Postgres versions support `CREATE DATABASE IF NOT EXISTS`,
    // but that’s non‐standard. We can do a try/ignore approach:
    let create_db_sql = format!("CREATE DATABASE {test_db};");
    match sqlx::query(&create_db_sql).execute(&mut conn).await {
        Ok(_) => {
            println!("Created test DB '{test_db}'.");
        }
        Err(e) => {
            // Check for "already exists" error code 42P04
            if let Some(db_err) = e.as_database_error() {
                if let Some(code) = db_err.code() {
                    if code == "42P04" {
                        // 42P04 => "duplicate_database"
                        println!("Test DB '{test_db}' already exists; ignoring.");
                    } else {
                        return Err(Error::Database(e));
                    }
                } else {
                    return Err(Error::Database(e));
                }
            } else {
                return Err(Error::Database(e));
            }
        }
    }

    Ok(())
}

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

/// Returns a migrated, empty test DB handle.
pub async fn setup_test_database() -> Result<Database, Error> {
    // 1) Make sure the test database exists:
    ensure_test_database_exists().await?;

    // 2) Now connect to `maowbot_test` and run migrations/clean:
    let pool = create_test_db_pool().await?;
    let db = Database::from_pool(pool);
    db.migrate().await?;
    clean_database(db.pool()).await?;

    Ok(db)
}