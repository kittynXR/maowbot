// File: maowbot-core/tests/integration/monthly_maintenance_tests.rs

use std::path::PathBuf;
use chrono::{NaiveDateTime, Utc};
use sqlx::{Executor, PgPool, Row};
use sqlx::postgres::{PgPoolOptions, PgConnectOptions};

use maowbot_core::{
    Error,
    db::Database,
    repositories::postgres::user_analysis::PostgresUserAnalysisRepository,
    tasks::monthly_maintenance::{
        maybe_run_monthly_maintenance, collect_missing_months, parse_year_month,
    },
    utils::time::to_epoch,
};
use maowbot_core::test_utils::helpers::setup_test_database;

/// Helper: Parse a date string ("YYYY-MM-DD HH:MM:SS") into epoch seconds.
fn parse_to_seconds(s: &str) -> Result<i64, Error> {
    let dt = NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S")?;
    Ok(dt.timestamp())
}

/// Demonstration of a specialized schema creation (unused here),
/// but left for reference if you want a custom test schema.
async fn create_test_schema(pool: &PgPool) -> Result<(), Error> {
    // Example: drop everything for a fresh start
    sqlx::query("DROP TABLE IF EXISTS chat_messages CASCADE;").execute(pool).await?;
    sqlx::query("DROP TABLE IF EXISTS users CASCADE;").execute(pool).await?;
    sqlx::query("DROP TABLE IF EXISTS maintenance_state;").execute(pool).await?;
    sqlx::query("DROP TABLE IF EXISTS user_analysis CASCADE;").execute(pool).await?;
    sqlx::query("DROP TABLE IF EXISTS user_analysis_history;").execute(pool).await?;

    // Create some base tables (non-partitioned or partitioned as you like)...
    // ...
    Ok(())
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

    // Should fail if the string is not zero-padded
    assert!(parse_year_month("2025-1").is_err());
    Ok(())
}

/// **FIXED**: We now insert messages from ~35 days ago and then call the monthly maintenance
/// so that they get archived out of the main table.
#[tokio::test]
async fn test_archive_one_month_no_attach() -> Result<(), Error> {
    let db = setup_test_database().await?;
    let analysis_repo = PostgresUserAnalysisRepository::new(db.pool().clone());

    // Insert dummy users.
    let now = Utc::now().timestamp();
    sqlx::query(r#"
        INSERT INTO users (user_id, created_at, last_seen, is_active)
        VALUES
            ($1, $2, $2, TRUE),
            ($3, $4, $4, TRUE)
    "#)
        .bind("u1")
        .bind(now)
        .bind("u2")
        .bind(now)
        .execute(db.pool())
        .await?;

    // Insert messages from ~35 days ago => ensures they belong to "last month"
    let older_ts = (Utc::now() - chrono::Duration::days(35)).timestamp();
    sqlx::query(r#"
        INSERT INTO chat_messages
            (message_id, platform, channel, user_id, message_text, timestamp, metadata)
        VALUES
            ($1, $2, $3, $4, $5, $6, $7),
            ($8, $9, $10, $11, $12, $13, $14)
    "#)
        .bind("A")
        .bind("twitch_helix")
        .bind("#chan")
        .bind("u1")
        .bind("HelloA")
        .bind(older_ts)
        .bind("{}")
        .bind("B")
        .bind("twitch_helix")
        .bind("#chan")
        .bind("u2")
        .bind("HelloB")
        .bind(older_ts)
        .bind("{}")
        .execute(db.pool())
        .await?;

    // Actually run the monthly maintenance/archival
    maybe_run_monthly_maintenance(&db, &analysis_repo).await?;

    // Now confirm the main DB table is empty for that older month
    let row = sqlx::query("SELECT COUNT(*) as cnt FROM chat_messages")
        .fetch_one(db.pool())
        .await?;
    let cnt: i64 = row.try_get("cnt")?;
    assert_eq!(cnt, 0, "Main table should be empty after archiving");

    // Optionally, verify they're in the archive:
    // (If your code writes them to a separate DB or a partition, adapt accordingly.)
    // For example, if you wrote them to chat_messages_archive in the same DB:
    let arch_count = sqlx::query("SELECT COUNT(*) as cnt FROM chat_messages_archive")
        .fetch_one(db.pool())
        .await?;
    let archived_cnt: i64 = arch_count.try_get("cnt")?;
    assert_eq!(archived_cnt, 2, "Archive should have 2 rows now");

    Ok(())
}

/// **FIXED**: Also insert older timestamps, then run monthly maintenance.
#[tokio::test]
async fn test_maybe_run_monthly_maintenance_integration() -> Result<(), Error> {
    let db = setup_test_database().await?;
    let analysis_repo = PostgresUserAnalysisRepository::new(db.pool().clone());

    // Insert two users.
    let now = Utc::now().timestamp();
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

    // Insert messages from ~35 days ago => belong to last month
    let older_ts = (Utc::now() - chrono::Duration::days(35)).timestamp();

    sqlx::query(r#"
        INSERT INTO chat_messages
            (message_id, platform, channel, user_id, message_text, timestamp, metadata)
        VALUES
            ($1, $2, $3, $4, $5, $6, $7)
    "#)
        .bind("A")
        .bind("twitch_helix")
        .bind("#chan")
        .bind("ua")
        .bind("HelloA")
        .bind(older_ts)
        .bind("{}")
        .execute(db.pool())
        .await?;

    sqlx::query(r#"
        INSERT INTO chat_messages
            (message_id, platform, channel, user_id, message_text, timestamp, metadata)
        VALUES
            ($1, $2, $3, $4, $5, $6, $7)
    "#)
        .bind("B")
        .bind("twitch_helix")
        .bind("#chan")
        .bind("ub")
        .bind("HelloB")
        .bind(older_ts)
        .bind("{}")
        .execute(db.pool())
        .await?;

    // Create "archives" dir or whichever path your code uses
    std::fs::create_dir_all("archives")?;
    let arch_file = PathBuf::from("archives").join("current_archive.db");
    if arch_file.exists() {
        std::fs::remove_file(&arch_file)?;
    }

    // Run monthly maintenance
    maybe_run_monthly_maintenance(&db, &analysis_repo).await?;

    // Confirm main table is now empty
    let row = sqlx::query("SELECT COUNT(*) as cnt FROM chat_messages")
        .fetch_one(db.pool())
        .await?;
    let cnt: i64 = row.try_get("cnt")?;
    assert_eq!(cnt, 0, "Main table should be empty after monthly maintenance");

    // Confirm we updated archived_until in maintenance_state
    let row2 = sqlx::query("SELECT state_value FROM maintenance_state WHERE state_key='archived_until'")
        .fetch_one(db.pool())
        .await?;
    let archived_until: String = row2.try_get("state_value")?;
    // e.g. "2025-02" if we are in February 2025:
    let current_year_month = Utc::now().format("%Y-%m").to_string();
    assert_eq!(
        archived_until,
        current_year_month,
        "State should record the current year-month"
    );

    db.pool().close().await;

    // If you connect to a separate “archive DB” logic, adapt accordingly.
    // Here we just check that the file was created:
    assert!(
        arch_file.exists(),
        "Should have created an archive file"
    );

    // Or if you wrote them to table chat_messages_archive in the same DB, check that:
    let connect_opts = PgConnectOptions::new()
        .host("localhost")
        .port(5432)
        .username("maow")
        .database("maowbot");
    let arch_pool = PgPoolOptions::new()
        .max_connections(1)
        .connect_with(connect_opts)
        .await?;

    let row3 = sqlx::query("SELECT COUNT(*) as cnt FROM chat_messages_archive")
        .fetch_one(&arch_pool)
        .await?;
    let archived_cnt: i64 = row3.try_get("cnt")?;
    assert_eq!(archived_cnt, 2, "Archive should have 2 rows");

    Ok(())
}
