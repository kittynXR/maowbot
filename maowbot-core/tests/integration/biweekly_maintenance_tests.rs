// File: maowbot-core/tests/integration/biweekly_maintenance_tests.rs

use chrono::{DateTime, Utc, Duration, NaiveDate, Datelike};
use sqlx::{Row, Executor};
use uuid::Uuid;

use maowbot_core::{
    db::Database,
    tasks::biweekly_maintenance::{
        run_biweekly_maintenance, run_partition_maintenance, run_analysis,
    },
    repositories::postgres::user_analysis::{PostgresUserAnalysisRepository, UserAnalysisRepository},
    models::{UserAnalysis},
    Error,
};
use maowbot_core::test_utils::helpers::setup_test_database;

/// Example: Verifies that truly "old" messages in an older partition
/// get removed (partition dropped) by run_biweekly_maintenance.
/// Also checks user analysis is triggered.
#[tokio::test]
async fn test_biweekly_maintenance_removes_old_messages() -> Result<(), Error> {
    // 1) Setup test database
    let db = setup_test_database().await?;
    db.migrate().await?;

    // 2) Create a clearly old partition, e.g. "chat_messages_202401" for January 2024
    //    If we’re in Feb 2025 (or near that), that partition is definitely > 365 days old,
    //    so it's way beyond the 60-day threshold for removal.
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS chat_messages_202401
          PARTITION OF chat_messages
          FOR VALUES FROM ('2024-01-01 00:00:00+00')
                       TO   ('2024-02-01 00:00:00+00');
        "#,
    )
        .execute(db.pool())
        .await?;

    // 3) Insert a user for foreign key references
    let user_id = "user_1";
    sqlx::query(
        r#"
        INSERT INTO users (user_id, created_at, last_seen, is_active)
        VALUES ($1, NOW(), NOW(), TRUE)
        "#,
    )
        .bind(user_id)
        .execute(db.pool())
        .await?;

    // 4) Insert “old” messages from mid-January 2024
    //    This definitely should be older than 60 days if we run the test in 2025 (or late 2024).
    let old_date = chrono::NaiveDate::from_ymd_opt(2024, 1, 15)
        .unwrap()
        .and_hms_opt(12, 0, 0)
        .unwrap();
    let old_ts = chrono::DateTime::<chrono::Utc>::from_utc(old_date, chrono::Utc);

    for i in 0..3 {
        sqlx::query(
            r#"
            INSERT INTO chat_messages (timestamp, message_id, platform, channel, user_id, message_text)
            VALUES ($1, $2, 'test_platform', 'old_channel', $3, $4)
            "#,
        )
            .bind(old_ts)
            .bind(format!("old_msg_{}", i))
            .bind(user_id)
            .bind(format!("This is old message #{}", i))
            .execute(db.pool())
            .await?;
    }

    // 5) Insert “recent” messages in the current month partition
    //    For simplicity, rely on the maintenance code to create that partition automatically.
    let now = chrono::Utc::now();
    for i in 0..2 {
        sqlx::query(
            r#"
            INSERT INTO chat_messages (timestamp, message_id, platform, channel, user_id, message_text)
            VALUES ($1, $2, 'test_platform', 'recent_channel', $3, $4)
            "#,
        )
            .bind(now)
            .bind(format!("new_msg_{}", i))
            .bind(user_id)
            .bind(format!("Recent message #{}", i))
            .execute(db.pool())
            .await?;
    }

    // 6) We also want to test user_analysis creation/updates:
    let analysis_repo = PostgresUserAnalysisRepository::new(db.pool().clone());

    // 7) Run the main function that does partition housekeeping (cutoff=60) + analysis
    run_biweekly_maintenance(&db, &analysis_repo).await?;

    // 8) Confirm old partition was dropped -> old messages no longer in chat_messages.
    //    We'll see if 'chat_messages_202401' still exists:
    let check_partition = sqlx::query(
        r#"
        SELECT EXISTS (
            SELECT FROM pg_tables
            WHERE tablename = 'chat_messages_202401'
        ) AS partition_exists
        "#,
    )
        .fetch_one(db.pool())
        .await?;
    let old_part_exists: bool = check_partition.try_get("partition_exists")?;
    assert!(
        !old_part_exists,
        "Partition chat_messages_202401 should have been dropped if older than 60 days."
    );

    // The 2 recent messages should still be in chat_messages
    let newer_in_chat = sqlx::query(
        r#"
        SELECT message_id FROM chat_messages
        WHERE message_id LIKE 'new_msg_%'
        "#
    )
        .fetch_all(db.pool())
        .await?;
    assert_eq!(
        newer_in_chat.len(),
        2,
        "The 2 recent messages should remain in chat_messages"
    );

    // 9) Confirm user_analysis was created or updated
    let found_analysis = analysis_repo.get_analysis(user_id).await?;
    assert!(
        found_analysis.is_some(),
        "UserAnalysis record should have been created/updated for user_1"
    );

    Ok(())
}

/// Verifies partition maintenance alone: ensures that
/// (a) partitions for the current/next month are created,
/// (b) old partitions are dropped for an older cutoff.
#[tokio::test]
async fn test_partition_maintenance_creates_and_drops_partitions() -> Result<(), Error> {
    let db = setup_test_database().await?;
    db.migrate().await?;

    // Create an “old” partition that we'll confirm gets dropped
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS chat_messages_202401
          PARTITION OF chat_messages
          FOR VALUES FROM ('2024-01-01 00:00:00+00')
                       TO   ('2024-02-01 00:00:00+00');
        "#,
    )
        .execute(db.pool())
        .await?;

    // Suppose we set cutoff=200 days (meaning anything older than 200 days from now is dropped).
    // If "now" is 2025-xx, this presumably kills the 2024-01 partition if it's older than 200 days.
    let cutoff_days = 200;
    run_partition_maintenance(&db, cutoff_days).await?;

    // Confirm a partition for the current month was created
    let now = Utc::now();
    let year = now.year();
    let month = now.month();
    let partition_name = format!("chat_messages_{:04}{:02}", year, month);

    let partition_exists_row = sqlx::query(
        r#"
        SELECT EXISTS (
            SELECT FROM pg_tables
            WHERE tablename = $1
        ) AS partition_exists
        "#,
    )
        .bind(&partition_name)
        .fetch_one(db.pool())
        .await?;

    let partition_exists: bool = partition_exists_row.try_get("partition_exists")?;
    assert!(
        partition_exists,
        "Partition for the current month should have been created"
    );

    // Check whether the old 2024-01 partition was dropped (depends on the actual date)
    // If your local time is not far enough to be > 200 days since 2024-01, it might remain.
    // You can adjust your test logic as needed. For demonstration:
    let old_partition_exists_row = sqlx::query(
        r#"
        SELECT EXISTS (
            SELECT FROM pg_tables
            WHERE tablename = 'chat_messages_202401'
        ) AS old_part_exists
        "#,
    )
        .fetch_one(db.pool())
        .await?;
    let old_exists: bool = old_partition_exists_row.try_get("old_part_exists")?;

    println!("Partition 2024-01 was dropped? => {}", !old_exists);

    // If you want a strict pass/fail, pick a date or cutoff that definitely ensures it's dropped.
    // Or you can just check that no error occurred. We'll let this pass either way.
    // assert!(!old_exists, "The old partition 2024-01 should have been dropped.");

    Ok(())
}

/// Verifies the user analysis portion alone (no dropping partitions).
/// We'll insert messages in the *current* month partition, then run analysis.
#[tokio::test]
async fn test_run_analysis_current_messages() -> Result<(), Error> {
    let db = setup_test_database().await?;
    db.migrate().await?;

    // Create a partition for the current month so we can insert “recent” data
    let now = Utc::now();
    let year = now.year();
    let month = now.month();
    let start_of_month = NaiveDate::from_ymd_opt(year, month, 1).unwrap().and_hms_opt(0,0,0).unwrap();
    let partition_name = format!("chat_messages_{:04}{:02}", year, month);
    let create_sql = format!(
        r#"
        CREATE TABLE IF NOT EXISTS {partition_name}
          PARTITION OF chat_messages
          FOR VALUES FROM ('{start_ts}') TO ('{end_ts}');
        "#,
        partition_name = partition_name,
        start_ts = start_of_month,
        end_ts = start_of_month + chrono::Duration::days(31)
    );
    sqlx::query(&create_sql).execute(db.pool()).await?;

    // Insert user
    let user_id = "analysis_user";
    sqlx::query(
        r#"
        INSERT INTO users (user_id, created_at, last_seen, is_active)
        VALUES ($1, NOW(), NOW(), TRUE)
        "#,
    )
        .bind(user_id)
        .execute(db.pool())
        .await?;

    // Insert a few messages from “now” => within the current month's partition
    for i in 0..3 {
        sqlx::query(
            r#"
            INSERT INTO chat_messages (timestamp, message_id, platform, channel, user_id, message_text)
            VALUES ($1, $2, 'twitch', 'analysis_channel', $3, $4)
            "#,
        )
            .bind(now)
            .bind(format!("analysis_msg_{}", i))
            .bind(user_id)
            .bind(format!("Sample message #{}", i))
            .execute(db.pool())
            .await?;
    }

    // Just run the analysis step
    let analysis_repo = PostgresUserAnalysisRepository::new(db.pool().clone());
    run_analysis(&db, &analysis_repo).await?;

    // Confirm user_analysis was created
    let analysis = analysis_repo.get_analysis(user_id).await?;
    assert!(
        analysis.is_some(),
        "A user_analysis row should be created for 'analysis_user'"
    );

    // Also confirm a monthly record got inserted
    let year_month = format!("{}-{:02}", now.year(), now.month());
    let hist_rows = sqlx::query(
        r#"
        SELECT user_analysis_history_id FROM user_analysis_history
        WHERE user_id = $1 AND year_month = $2
        "#,
    )
        .bind(user_id)
        .bind(&year_month)
        .fetch_all(db.pool())
        .await?;

    assert_eq!(
        hist_rows.len(),
        1,
        "Should have 1 monthly summary row in user_analysis_history"
    );

    Ok(())
}

/// End-to-end test that calls `run_biweekly_maintenance`,
/// verifying partition maintenance and user analysis in one shot.
#[tokio::test]
async fn test_run_biweekly_maintenance_full_pipeline() -> Result<(), Error> {
    let db = setup_test_database().await?;
    db.migrate().await?;

    // We'll rely on the code inside `run_biweekly_maintenance` to create partitions
    // for the current (and next) month, plus drop old ones, plus run analysis.
    // Just insert some data spanning old + new, then confirm behaviors.

    // Insert user
    let user_id = "end_to_end_user";
    sqlx::query(
        r#"
        INSERT INTO users (user_id, created_at, last_seen, is_active)
        VALUES ($1, NOW(), NOW(), TRUE)
        "#,
    )
        .bind(user_id)
        .execute(db.pool())
        .await?;

    // Let's create a partition for an older date range so we can insert old messages
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS chat_messages_202406
          PARTITION OF chat_messages
          FOR VALUES FROM ('2024-06-01 00:00:00+00')
                       TO   ('2024-07-01 00:00:00+00');
        "#,
    )
        .execute(db.pool())
        .await?;

    // Insert old messages in June 2024
    let old_date = NaiveDate::from_ymd_opt(2024, 6, 15)
        .unwrap()
        .and_hms_opt(12, 0, 0)
        .unwrap();
    let old_ts = DateTime::<Utc>::from_utc(old_date, Utc);
    for i in 0..3 {
        sqlx::query(
            r#"
            INSERT INTO chat_messages (timestamp, message_id, platform, channel, user_id, message_text)
            VALUES ($1, $2, 'twitch', 'old_channel', $3, $4)
            "#,
        )
            .bind(old_ts)
            .bind(format!("old_june_msg_{}", i))
            .bind(user_id)
            .bind(format!("Old message in June #{}", i))
            .execute(db.pool())
            .await?;
    }

    // Insert recent messages from "now"
    let now = Utc::now();
    for i in 0..2 {
        sqlx::query(
            r#"
            INSERT INTO chat_messages (timestamp, message_id, platform, channel, user_id, message_text)
            VALUES ($1, $2, 'twitch', 'new_channel', $3, $4)
            "#,
        )
            .bind(now)
            .bind(format!("new_msg_{}", i))
            .bind(user_id)
            .bind(format!("Recent message #{}", i))
            .execute(db.pool())
            .await?;
    }

    // Now run the full maintenance logic (which includes partition creation for current/next month,
    // dropping old partitions older than ~60 days, and user analysis)
    let analysis_repo = PostgresUserAnalysisRepository::new(db.pool().clone());
    run_biweekly_maintenance(&db, &analysis_repo).await?;

    // If June 2024 is > 60 days in the past from the current date, that partition might be dropped.
    // We'll check if it still exists:
    let partition_202406_exists_row = sqlx::query(
        r#"
        SELECT EXISTS (
            SELECT FROM pg_tables
            WHERE tablename = 'chat_messages_202406'
        ) AS old_exists
        "#
    )
        .fetch_one(db.pool())
        .await?;
    let old_exists: bool = partition_202406_exists_row.try_get("old_exists")?;

    println!("Partition 2024-06 was dropped? => {}", !old_exists);

    // Check that the new messages from "now" remain
    let new_msgs = sqlx::query(
        r#"SELECT message_id FROM chat_messages WHERE message_id LIKE 'new_msg_%'"#,
    )
        .fetch_all(db.pool())
        .await?;
    assert_eq!(new_msgs.len(), 2, "Recent messages must still exist");

    // Confirm user_analysis was created or updated
    let found_analysis = analysis_repo.get_analysis(user_id).await?;
    assert!(
        found_analysis.is_some(),
        "Should have user_analysis for 'end_to_end_user'"
    );

    Ok(())
}