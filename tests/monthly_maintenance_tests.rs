// tests/monthly_maintenance_tests.rs

use std::path::PathBuf;
use tempfile::NamedTempFile;
use tokio::fs;
use chrono::NaiveDateTime;
use sqlx::{SqlitePool, Row};
use sqlx::sqlite::{SqlitePoolOptions, SqliteConnectOptions};
use anyhow::Result;
use maowbot::Error;
use maowbot::Database;
use maowbot::repositories::sqlite::user_analysis::SqliteUserAnalysisRepository;
use maowbot::tasks::monthly_maintenance::{
    archive_one_month_no_attach, maybe_run_monthly_maintenance, collect_missing_months, parse_year_month,
};
use maowbot::utils::time::to_epoch;

/// Helper: Parse a date string ("YYYY-MM-DD HH:MM:SS") into epoch seconds.
fn parse_to_seconds(s: &str) -> Result<i64> {
    let dt = NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S")?;
    Ok(dt.timestamp())
}

async fn create_single_conn_pool(db_path: &str) -> Result<SqlitePool> {
    let abs_path = std::env::current_dir()?.join(db_path);
    let connect_opts = SqliteConnectOptions::new()
        .filename(abs_path)
        .create_if_missing(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(connect_opts)
        .await?;
    // Enforce foreign keys.
    sqlx::query("PRAGMA foreign_keys = ON;")
        .execute(&pool)
        .await?;
    Ok(pool)
}

async fn create_test_schema(pool: &SqlitePool) -> Result<()> {
    // Create minimal tables using INTEGER timestamps.
    sqlx::query(r#"
        CREATE TABLE IF NOT EXISTS users (
            user_id TEXT PRIMARY KEY,
            global_username TEXT,
            created_at INTEGER,
            last_seen INTEGER,
            is_active BOOLEAN NOT NULL DEFAULT 1
        );
    "#).execute(pool).await?;

    sqlx::query(r#"
        CREATE TABLE IF NOT EXISTS chat_messages (
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
        CREATE TABLE IF NOT EXISTS maintenance_state (
            state_key TEXT PRIMARY KEY,
            state_value TEXT
        );
    "#).execute(pool).await?;

    sqlx::query(r#"
        CREATE TABLE IF NOT EXISTS user_analysis (
            user_analysis_id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL,
            spam_score REAL NOT NULL DEFAULT 0,
            intelligibility_score REAL NOT NULL DEFAULT 0,
            quality_score REAL NOT NULL DEFAULT 0,
            horni_score REAL NOT NULL DEFAULT 0,
            ai_notes TEXT,
            moderator_notes TEXT,
            created_at INTEGER NOT NULL DEFAULT (strftime('%s','now')),
            updated_at INTEGER NOT NULL DEFAULT (strftime('%s','now')),
            FOREIGN KEY (user_id) REFERENCES users(user_id)
        );
    "#).execute(pool).await?;

    sqlx::query(r#"
        CREATE TABLE IF NOT EXISTS user_analysis_history (
            user_analysis_history_id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL,
            year_month TEXT NOT NULL,
            spam_score REAL NOT NULL DEFAULT 0,
            intelligibility_score REAL NOT NULL DEFAULT 0,
            quality_score REAL NOT NULL DEFAULT 0,
            horni_score REAL NOT NULL DEFAULT 0,
            ai_notes TEXT,
            created_at INTEGER NOT NULL DEFAULT (strftime('%s','now')),
            FOREIGN KEY (user_id) REFERENCES users(user_id)
        );
    "#).execute(pool).await?;

    Ok(())
}

async fn create_test_database(db_path: &str) -> Result<Database> {
    let pool = create_single_conn_pool(db_path).await?;
    Ok(Database::from_pool(pool))
}

#[tokio::test]
async fn test_collect_missing_months_logic() -> Result<()> {
    let months = collect_missing_months(Some("2024-11"), "2025-01")?;
    assert_eq!(months, vec!["2024-12", "2025-01"]);
    let none_before = collect_missing_months(None, "2025-04")?;
    assert_eq!(none_before, vec!["2025-04"]);
    Ok(())
}

#[tokio::test]
async fn test_parse_year_month() -> Result<()> {
    let (y, m) = parse_year_month("2025-01")?;
    assert_eq!(y, 2025);
    assert_eq!(m, 1);
    assert!(parse_year_month("2025-1").is_err());
    Ok(())
}

#[tokio::test]
async fn test_archive_one_month_no_attach() -> anyhow::Result<()> {
    let tmp_main = NamedTempFile::new()?;
    let main_db_path = tmp_main.path().display().to_string();
    let db = create_test_database(&main_db_path).await?;
    create_test_schema(db.pool()).await?;

    // Insert dummy users.
    sqlx::query("INSERT INTO users (user_id) VALUES ('u1'), ('u2');")
        .execute(db.pool()).await?;

    // Insert messages with a known timestamp.
    let msg_ts = parse_to_seconds("2023-01-15 10:30:00")?;
    sqlx::query(r#"
        INSERT INTO chat_messages (message_id, platform, channel, user_id, message_text, timestamp, metadata)
        VALUES
          ('A','twitch','#chan','u1','HelloA',?, '{}'),
          ('B','twitch','#chan','u2','HelloB',?, '{}')
    "#)
        .bind(msg_ts)
        .bind(msg_ts)
        .execute(db.pool()).await?;

    let archive_file = NamedTempFile::new()?;
    let archive_path = archive_file.path();

    archive_one_month_no_attach(
        &db,
        "2023-01-01 00:00:00",
        "2023-02-01 00:00:00",
        archive_path
    ).await?;

    // Confirm main DB is now empty.
    let row = sqlx::query("SELECT COUNT(*) as cnt FROM chat_messages")
        .fetch_one(db.pool())
        .await?;
    let cnt: i64 = row.try_get("cnt")?;
    assert_eq!(cnt, 0);

    db.pool().close().await;

    // Confirm archive DB has the messages.
    let arch_opts = SqliteConnectOptions::new()
        .filename(&archive_path)
        .create_if_missing(false);
    let arch_pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(arch_opts)
        .await?;

    let row2 = sqlx::query("SELECT COUNT(*) as cnt FROM chat_messages")
        .fetch_one(&arch_pool).await?;
    let cnt2: i64 = row2.try_get("cnt")?;
    assert_eq!(cnt2, 2);

    Ok(())
}

#[tokio::test]
async fn test_maybe_run_monthly_maintenance_integration() -> Result<()> {
    let tmpfile = NamedTempFile::new()?;
    let main_db_path = tmpfile.path().display().to_string();
    let db = create_test_database(&main_db_path).await?;
    create_test_schema(db.pool()).await?;

    // Insert two users.
    sqlx::query("INSERT INTO users (user_id) VALUES ('ua'), ('ub');")
        .execute(db.pool())
        .await?;

    let ts_jan10 = parse_to_seconds("2025-01-10 12:00:00")?;
    sqlx::query(r#"
      INSERT INTO chat_messages (message_id, platform, channel, user_id, message_text, timestamp, metadata)
      VALUES
        ('msg001','twitch','#chan','ua','HelloUA',?, '{}'),
        ('msg002','twitch','#chan','ub','HelloUB',?, '{}')
    "#)
        .bind(ts_jan10)
        .bind(ts_jan10)
        .execute(db.pool())
        .await?;

    std::fs::create_dir_all("archives")?;
    let arch_file = PathBuf::from("archives").join("2025-01_archive.db");
    if arch_file.exists() {
        std::fs::remove_file(&arch_file)?;
    }

    let analysis_repo = SqliteUserAnalysisRepository::new(db.pool().clone());
    maowbot::tasks::monthly_maintenance::maybe_run_monthly_maintenance(&db, &analysis_repo).await?;

    let row = sqlx::query("SELECT COUNT(*) as cnt FROM chat_messages")
        .fetch_one(db.pool())
        .await?;
    let cnt: i64 = row.try_get("cnt")?;
    assert_eq!(cnt, 0);

    let row2 = sqlx::query("SELECT state_value FROM maintenance_state WHERE state_key='archived_until'")
        .fetch_one(db.pool())
        .await?;
    let archived_until: String = row2.try_get("state_value")?;
    assert_eq!(archived_until, "2025-01");

    db.pool().close().await;
    assert!(arch_file.exists(), "Should have created january archive");

    let arch_opts = SqliteConnectOptions::new()
        .filename(&arch_file)
        .create_if_missing(false);
    let arch_pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(arch_opts)
        .await?;

    let row3 = sqlx::query("SELECT COUNT(*) as cnt FROM chat_messages")
        .fetch_one(&arch_pool)
        .await?;
    let archived_cnt: i64 = row3.try_get("cnt")?;
    assert_eq!(archived_cnt, 2);

    Ok(())
}