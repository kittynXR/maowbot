// File: maowbot-core/tests/integration/monthly_maintenance_tests.rs
//
// These integration tests verify that your monthly maintenance routine archives rows
// from chat_messages that are from a fully completed (old) month.
// In these tests we “reset” the maintenance state so that archiving is forced,
// and we insert rows with timestamps computed relative to a fixed point in time.
// We assume that rows with a timestamp from two months ago (e.g. the 15th day at noon)
// should be archived, while rows from the current month (e.g. the 10th day at noon)
// remain in chat_messages.
// Adjust table names and SQL statements if your schema is different.

use chrono::{Datelike, TimeZone, Utc};
use sqlx::{Row, Executor};
use uuid::Uuid;

use maowbot_core::{
    Error,
    db::Database,
    // The function under test.
    tasks::monthly_maintenance::maybe_run_monthly_maintenance,
    repositories::postgres::user_analysis::PostgresUserAnalysisRepository,
    // Helper to set up a fresh, migrated test database.
    test_utils::helpers::setup_test_database,
};

/// Returns a timestamp (in seconds) from the 15th day at noon, two months ago.
/// This should guarantee that the data is from a fully completed month.
fn two_months_ago() -> i64 {
    let now = Utc::now();
    if now.month() > 2 {
        Utc.with_ymd_and_hms(now.year(), now.month() - 2, 15, 12, 0, 0)
            .unwrap()
            .timestamp()
    } else {
        // Cross-year boundary:
        let new_year = now.year() - 1;
        let new_mon = 12 + now.month() as u32 - 2;
        Utc.with_ymd_and_hms(new_year, new_mon, 15, 12, 0, 0)
            .unwrap()
            .timestamp()
    }
}

/// Returns a timestamp from the current month.
/// We choose the 10th day at noon.
fn current_month_time() -> i64 {
    let now = Utc::now();
    Utc.with_ymd_and_hms(now.year(), now.month(), 10, 12, 0, 0)
        .unwrap()
        .timestamp()
}

/// Resets the maintenance state by deleting any row in the maintenance_state table
/// with state_key = 'archived_until'. This forces the maintenance routine to run its archiving logic.
async fn reset_maintenance_state(db: &Database) -> Result<(), Error> {
    sqlx::query("DELETE FROM maintenance_state WHERE state_key = 'archived_until';")
        .execute(db.pool())
        .await?;
    Ok(())
}

/// Cleans (truncates) the chat_messages_archive table.
/// (Your global clean_database routine may not clear the archive table.)
async fn clean_archive_table(db: &Database) -> Result<(), Error> {
    sqlx::query("TRUNCATE TABLE chat_messages_archive RESTART IDENTITY CASCADE;")
        .execute(db.pool())
        .await?;
    Ok(())
}

/// Inserts a chat message into chat_messages and ensures that a corresponding user row exists.
/// Adjust the SQL if your schema differs.
async fn insert_chat_message(
    db: &Database,
    user_id: &str,
    timestamp: i64,
    text: &str,
) -> Result<(), Error> {
    // Insert a user row if not already present.
    sqlx::query(
        r#"
        INSERT INTO users (user_id, created_at, last_seen, is_active)
        VALUES ($1, $2, $2, TRUE)
        ON CONFLICT (user_id) DO NOTHING;
        "#,
    )
        .bind(user_id)
        .bind(timestamp)
        .execute(db.pool())
        .await?;

    // Insert a chat message.
    let msg_id = Uuid::new_v4().to_string();
    sqlx::query(
        r#"
        INSERT INTO chat_messages (
            message_id,
            platform,
            channel,
            user_id,
            message_text,
            timestamp,
            metadata
        )
        VALUES ($1, 'twitch_helix', 'some_channel', $2, $3, $4, '{}');
        "#,
    )
        .bind(&msg_id)
        .bind(user_id)
        .bind(text)
        .bind(timestamp)
        .execute(db.pool())
        .await?;
    Ok(())
}

/// Counts the number of rows in a given table.
async fn count_rows(db: &Database, table: &str) -> Result<i64, Error> {
    let query = format!("SELECT COUNT(*) as cnt FROM {}", table);
    let row = sqlx::query(&query).fetch_one(db.pool()).await?;
    Ok(row.try_get("cnt")?)
}

/// Test that when one row from two months ago and one row from the current month
/// are inserted, after maintenance only the current row remains in chat_messages,
/// and the old row is archived.
#[tokio::test]
async fn test_archive_old_data() -> Result<(), Error> {
    let db = setup_test_database().await?;
    // Reset maintenance state and clean archive table.
    reset_maintenance_state(&db).await?;
    clean_archive_table(&db).await?;
    let analysis_repo = PostgresUserAnalysisRepository::new(db.pool().clone());

    let old_timestamp = two_months_ago();
    let current_timestamp = current_month_time();

    insert_chat_message(&db, "user_old", old_timestamp, "Old message").await?;
    insert_chat_message(&db, "user_new", current_timestamp, "New message").await?;

    // Sanity check: main table should have 2 rows.
    assert_eq!(count_rows(&db, "chat_messages").await?, 2);

    // Run the maintenance routine.
    maybe_run_monthly_maintenance(&db, &analysis_repo).await?;

    let main_count = count_rows(&db, "chat_messages").await?;
    let archive_count = count_rows(&db, "chat_messages_archive").await?;

    // We expect that only the current row remains in chat_messages.
    assert_eq!(
        main_count,
        1,
        "Only current messages should remain in chat_messages"
    );
    // And the old row should be in the archive.
    assert_eq!(
        archive_count,
        1,
        "Old messages should be archived in chat_messages_archive"
    );

    Ok(())
}

/// Test that if all inserted rows are current (timestamp >= current month time),
/// then maintenance does not archive any rows.
#[tokio::test]
async fn test_no_archiving_if_all_current() -> Result<(), Error> {
    let db = setup_test_database().await?;
    reset_maintenance_state(&db).await?;
    clean_archive_table(&db).await?;
    let analysis_repo = PostgresUserAnalysisRepository::new(db.pool().clone());

    let current_timestamp = current_month_time();

    insert_chat_message(&db, "user1", current_timestamp, "Message 1").await?;
    insert_chat_message(&db, "user2", current_timestamp + 100, "Message 2").await?;

    maybe_run_monthly_maintenance(&db, &analysis_repo).await?;

    let main_count = count_rows(&db, "chat_messages").await?;
    let archive_count = count_rows(&db, "chat_messages_archive").await?;

    assert_eq!(
        main_count,
        2,
        "All current messages should remain in chat_messages"
    );
    assert_eq!(
        archive_count,
        0,
        "No messages should be archived when all are current"
    );

    Ok(())
}

/// Test that multiple old rows are archived in one run.
#[tokio::test]
async fn test_archive_multiple_old_rows() -> Result<(), Error> {
    let db = setup_test_database().await?;
    reset_maintenance_state(&db).await?;
    clean_archive_table(&db).await?;
    let analysis_repo = PostgresUserAnalysisRepository::new(db.pool().clone());

    let current_timestamp = current_month_time();
    // Insert three old rows using two_months_ago() and slight offsets.
    let old1 = two_months_ago();
    let old2 = two_months_ago() - 24 * 3600 * 10;  // 10 days earlier
    let old3 = two_months_ago() - 24 * 3600 * 20;  // 20 days earlier

    insert_chat_message(&db, "user1", old1, "Old message 1").await?;
    insert_chat_message(&db, "user2", old2, "Old message 2").await?;
    insert_chat_message(&db, "user3", old3, "Old message 3").await?;

    // Also insert one current row.
    insert_chat_message(&db, "user4", current_timestamp, "Current message").await?;

    // Initially, main table should have 4 rows.
    assert_eq!(count_rows(&db, "chat_messages").await?, 4);

    maybe_run_monthly_maintenance(&db, &analysis_repo).await?;

    let main_count = count_rows(&db, "chat_messages").await?;
    let archive_count = count_rows(&db, "chat_messages_archive").await?;

    assert_eq!(
        main_count,
        1,
        "After maintenance, only current messages should remain in chat_messages"
    );
    assert_eq!(
        archive_count,
        3,
        "After maintenance, all old messages should be archived in chat_messages_archive"
    );

    Ok(())
}

/// Test idempotency: running maintenance twice should yield the same result.
#[tokio::test]
async fn test_idempotent_maintenance() -> Result<(), Error> {
    let db = setup_test_database().await?;
    reset_maintenance_state(&db).await?;
    clean_archive_table(&db).await?;
    let analysis_repo = PostgresUserAnalysisRepository::new(db.pool().clone());

    let old_timestamp = two_months_ago();

    insert_chat_message(&db, "user_old", old_timestamp, "Old message").await?;

    maybe_run_monthly_maintenance(&db, &analysis_repo).await?;
    let main_count_first = count_rows(&db, "chat_messages").await?;
    let archive_count_first = count_rows(&db, "chat_messages_archive").await?;

    maybe_run_monthly_maintenance(&db, &analysis_repo).await?;
    let main_count_second = count_rows(&db, "chat_messages").await?;
    let archive_count_second = count_rows(&db, "chat_messages_archive").await?;

    assert_eq!(
        main_count_first,
        main_count_second,
        "Main table count should remain stable on repeated runs"
    );
    assert_eq!(
        archive_count_first,
        archive_count_second,
        "Archive table count should remain stable on repeated runs"
    );

    Ok(())
}
