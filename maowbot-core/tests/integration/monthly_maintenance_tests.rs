// tests/integration/monthly_maintenance_tests.rs
use std::path::PathBuf;
use tempfile::NamedTempFile;
use chrono::{NaiveDateTime, Utc};
use sqlx::{Executor, PgPool, Row};
use sqlx::postgres::{PgPoolOptions, PgConnectOptions};
use maowbot_core::Error;
use maowbot_core::Database;
use maowbot_core::repositories::postgres::user_analysis::PostgresUserAnalysisRepository;
use maowbot_core::tasks::monthly_maintenance::{
    maybe_run_monthly_maintenance, collect_missing_months, parse_year_month,
};
use maowbot_core::utils::time::to_epoch;

/// Helper: Parse a date string ("YYYY-MM-DD HH:MM:SS") into epoch seconds.
fn parse_to_seconds(s: &str) -> Result<i64, Error> {
    let dt = NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S")?;
    Ok(dt.timestamp())
}

async fn create_single_conn_pool(_db_path: &str) -> Result<PgPool, Error> {
    // Use a dedicated test database.
    let connect_opts = PgConnectOptions::new()
        .host("localhost")
        .port(5432)
        .username("maow")
        .database("maowbot"); // ensure this test database exists and is dedicated for testing
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect_with(connect_opts)
        .await?;
    Ok(pool)
}

async fn create_test_schema(pool: &PgPool) -> Result<(), Error> {
    // Drop tables if they exist so we get a clean slate.
    sqlx::query("DROP TABLE IF EXISTS chat_messages CASCADE;").execute(pool).await?;
    sqlx::query("DROP TABLE IF EXISTS users CASCADE;").execute(pool).await?;
    sqlx::query("DROP TABLE IF EXISTS maintenance_state;").execute(pool).await?;
    sqlx::query("DROP TABLE IF EXISTS user_analysis CASCADE;").execute(pool).await?;
    sqlx::query("DROP TABLE IF EXISTS user_analysis_history;").execute(pool).await?;

    // Create the tables as defined (non-partitioned for test simplicity)
    sqlx::query(r#"
        CREATE TABLE users (
            user_id TEXT PRIMARY KEY,
            global_username TEXT,
            created_at INTEGER NOT NULL DEFAULT (EXTRACT(EPOCH FROM NOW())::integer),
            last_seen INTEGER NOT NULL DEFAULT (EXTRACT(EPOCH FROM NOW())::integer),
            is_active BOOLEAN NOT NULL DEFAULT TRUE
        );
    "#).execute(pool).await?;

    sqlx::query(r#"
        CREATE TABLE chat_messages (
            message_id TEXT PRIMARY KEY,
            platform TEXT NOT NULL,
            channel TEXT NOT NULL,
            user_id TEXT NOT NULL,
            message_text TEXT NOT NULL,
            timestamp INTEGER NOT NULL,
            metadata TEXT,
            FOREIGN KEY (user_id) REFERENCES users(user_id)
        );
    "#).execute(pool).await?;

    sqlx::query(r#"
        CREATE TABLE maintenance_state (
            state_key TEXT PRIMARY KEY,
            state_value TEXT
        );
    "#).execute(pool).await?;

    sqlx::query(r#"
        CREATE TABLE user_analysis (
            user_analysis_id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL,
            spam_score REAL NOT NULL DEFAULT 0,
            intelligibility_score REAL NOT NULL DEFAULT 0,
            quality_score REAL NOT NULL DEFAULT 0,
            horni_score REAL NOT NULL DEFAULT 0,
            ai_notes TEXT,
            moderator_notes TEXT,
            created_at INTEGER NOT NULL DEFAULT (EXTRACT(EPOCH FROM NOW())::integer),
            updated_at INTEGER NOT NULL DEFAULT (EXTRACT(EPOCH FROM NOW())::integer),
            FOREIGN KEY (user_id) REFERENCES users(user_id)
        );
    "#).execute(pool).await?;

    sqlx::query(r#"
        CREATE TABLE user_analysis_history (
            user_analysis_history_id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL,
            year_month TEXT NOT NULL,
            spam_score REAL NOT NULL DEFAULT 0,
            intelligibility_score REAL NOT NULL DEFAULT 0,
            quality_score REAL NOT NULL DEFAULT 0,
            horni_score REAL NOT NULL DEFAULT 0,
            ai_notes TEXT,
            created_at INTEGER NOT NULL DEFAULT (EXTRACT(EPOCH FROM NOW())::integer),
            FOREIGN KEY (user_id) REFERENCES users(user_id)
        );
    "#).execute(pool).await?;

    Ok(())
}

async fn create_test_database(_db_path: &str) -> Result<Database, Error> {
    let pool = create_single_conn_pool(_db_path).await?;
    Ok(Database::from_pool(pool))
}

#[tokio::test]
async fn test_collect_missing_months_logic() -> Result<(), Error> {
    let months = collect_missing_months(Some("2024-11"), "2025-01")?;
    assert_eq!(months, vec!["2024-12", "2025-01"]);
    let none_before = collect_missing_months(None, "2025-04")?;
    assert_eq!(none_before, vec!["2025-04"]);
    Ok(())
}

#[tokio::test]
async fn test_parse_year_month() -> Result<(), Error> {
    let (y, m) = parse_year_month("2025-01")?;
    assert_eq!(y, 2025);
    assert_eq!(m, 1);
    assert!(parse_year_month("2025-1").is_err());
    Ok(())
}

#[tokio::test]
async fn test_archive_one_month_no_attach() -> Result<(), Error> {
    // We'll insert rows using today's timestamp so they fit into the current partition.
    let tmp_main = NamedTempFile::new()?;
    let main_db_path = tmp_main.path().display().to_string();
    let db = create_test_database(&main_db_path).await?;
    create_test_schema(db.pool()).await?;

    // Insert dummy users.
    let now = Utc::now().timestamp();
    sqlx::query(r#"
            INSERT INTO users (user_id, created_at, last_seen, is_active)
            VALUES ($1, $2, $2, TRUE),
                   ($3, $4, $4, TRUE)
        "#)
        .bind("u1")
        .bind(now)
        .bind("u2")
        .bind(now)
        .execute(db.pool())
        .await?;

    // Insert messages with today's timestamp.
    let msg_ts = Utc::now().timestamp();
    sqlx::query(
        r#"
            INSERT INTO chat_messages
              (message_id, platform, channel, user_id, message_text, timestamp, metadata)
            VALUES
              ($1, $2, $3, $4, $5, $6, $7),
              ($8, $9, $10, $11, $12, $13, $14)
        "#
    )
        .bind("A")
        .bind("twitch_helix")
        .bind("#chan")
        .bind("u1")
        .bind("HelloA")
        .bind(msg_ts)
        .bind("{}")
        .bind("B")
        .bind("twitch_helix")
        .bind("#chan")
        .bind("u2")
        .bind("HelloB")
        .bind(msg_ts)
        .bind("{}")
        .execute(db.pool())
        .await?;

    // Confirm main DB is now empty.
    let row = sqlx::query("SELECT COUNT(*) as cnt FROM chat_messages")
        .fetch_one(db.pool())
        .await?;
    let cnt: i64 = row.try_get("cnt")?;
    assert_eq!(cnt, 0, "Main table should be empty after archiving");

    db.pool().close().await;

    // Connect to the archive database.
    let connect_opts = PgConnectOptions::new()
        .host("localhost")
        .port(5432)
        .username("maow")
        .database("maowbot"); // Use a separate test DB for the archive
    let arch_pool = PgPoolOptions::new()
        .max_connections(1)
        .connect_with(connect_opts)
        .await?;

    // Confirm archive DB now has the messages.
    let row2 = sqlx::query("SELECT COUNT(*) as cnt FROM chat_messages")
        .fetch_one(&arch_pool)
        .await?;
    let cnt2: i64 = row2.try_get("cnt")?;
    assert_eq!(cnt2, 2, "Archive should have 2 rows");

    Ok(())
}

#[tokio::test]
async fn test_maybe_run_monthly_maintenance_integration() -> Result<(), Error> {
    // We'll insert data using today's timestamp.
    let tmpfile = NamedTempFile::new()?;
    let main_db_path = tmpfile.path().display().to_string();
    let db = create_test_database(&main_db_path).await?;
    create_test_schema(db.pool()).await?;

    let now = Utc::now().timestamp();
    // Insert two users.
    sqlx::query(r#"
            INSERT INTO users (user_id, created_at, last_seen, is_active)
            VALUES ($1, $2, $2, TRUE),
                   ($3, $4, $4, TRUE)
        "#)
        .bind("ua")
        .bind(now)
        .bind("ub")
        .bind(now)
        .execute(db.pool())
        .await?;

    // Use today's timestamp for the messages.
    let msg_ts = Utc::now().timestamp();
    // Insert first row:
    sqlx::query(
        r#"
            INSERT INTO chat_messages
                (message_id, platform, channel, user_id, message_text, timestamp, metadata)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
        "#
    )
        .bind("A")
        .bind("twitch_helix")
        .bind("#chan")
        .bind("ua")
        .bind("HelloA")
        .bind(msg_ts)
        .bind("{}")
        .execute(db.pool())
        .await?;

    // Insert second row:
    sqlx::query(
        r#"
            INSERT INTO chat_messages
                (message_id, platform, channel, user_id, message_text, timestamp, metadata)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
        "#
    )
        .bind("B")
        .bind("twitch_helix")
        .bind("#chan")
        .bind("ub")
        .bind("HelloB")
        .bind(msg_ts)
        .bind("{}")
        .execute(db.pool())
        .await?;

    std::fs::create_dir_all("archives")?;
    let arch_file = PathBuf::from("archives").join("current_archive.db");
    if arch_file.exists() {
        std::fs::remove_file(&arch_file)?;
    }

    let analysis_repo = PostgresUserAnalysisRepository::new(db.pool().clone());
    maowbot_core::tasks::monthly_maintenance::maybe_run_monthly_maintenance(&db, &analysis_repo).await?;

    let row = sqlx::query("SELECT COUNT(*) as cnt FROM chat_messages")
        .fetch_one(db.pool())
        .await?;
    let cnt: i64 = row.try_get("cnt")?;
    assert_eq!(cnt, 0, "Main table should be empty after monthly maintenance");

    let row2 = sqlx::query("SELECT state_value FROM maintenance_state WHERE state_key='archived_until'")
        .fetch_one(db.pool())
        .await?;
    let archived_until: String = row2.try_get("state_value")?;
    let current_year_month = Utc::now().format("%Y-%m").to_string();
    assert_eq!(archived_until, current_year_month, "State should record current year-month");

    db.pool().close().await;
    assert!(arch_file.exists(), "Should have created current archive");

    let connect_opts = PgConnectOptions::new()
        .host("localhost")
        .port(5432)
        .username("maow")
        .database("maowbot");
    let arch_pool = PgPoolOptions::new()
        .max_connections(1)
        .connect_with(connect_opts)
        .await?;

    let row3 = sqlx::query("SELECT COUNT(*) as cnt FROM chat_messages")
        .fetch_one(&arch_pool)
        .await?;
    let archived_cnt: i64 = row3.try_get("cnt")?;
    assert_eq!(archived_cnt, 2, "Archive should have 2 rows");

    Ok(())
}