use chrono::{DateTime, Utc, Duration, Datelike};
use sqlx::{Row, Executor};
use uuid::Uuid;

use maowbot_core::{
    db::Database,
    tasks::biweekly_maintenance::{
        run_biweekly_maintenance, run_partition_maintenance, run_archive_and_analysis,
    },
    repositories::postgres::user_analysis::{PostgresUserAnalysisRepository, UserAnalysisRepository},
    models::UserAnalysis,
    Error,
};

use maowbot_core::test_utils::helpers::setup_test_database;

#[tokio::test]
async fn test_biweekly_maintenance_archives_old_messages() -> Result<(), Error> {
    // 1) Set up the test database
    let db = setup_test_database().await?;
    db.migrate().await?;

    // 2) Insert a user so foreign key constraints pass
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

    // 3) Insert chat messages:
    //    - 3 messages older than 40 days
    //    - 2 messages from about 10 days ago (should remain)
    let now = Utc::now();
    let older_cutoff = now - Duration::days(40);
    let newer_cutoff = now - Duration::days(10);

    // Insert older messages
    for i in 0..3 {
        sqlx::query(
            r#"
            INSERT INTO chat_messages (
                message_id, platform, channel, user_id,
                message_text, timestamp, metadata
            )
            VALUES ($1, $2, $3, $4, $5, $6, NULL)
            "#,
        )
            .bind(format!("old_msg_{}", i))
            .bind("test_platform")
            .bind("test_channel")
            .bind(user_id)
            .bind(format!("Old message #{}", i))
            .bind(older_cutoff)
            .execute(db.pool())
            .await?;
    }

    // Insert newer messages
    for i in 0..2 {
        sqlx::query(
            r#"
            INSERT INTO chat_messages (
                message_id, platform, channel, user_id,
                message_text, timestamp, metadata
            )
            VALUES ($1, $2, $3, $4, $5, $6, NULL)
            "#,
        )
            .bind(format!("new_msg_{}", i))
            .bind("test_platform")
            .bind("test_channel")
            .bind(user_id)
            .bind(format!("New message #{}", i))
            .bind(newer_cutoff)
            .execute(db.pool())
            .await?;
    }

    // 4) We also want to test user_analysis creation/updates:
    //    Insert an existing user_analysis or rely on the function to create it.
    //    We'll rely on the summary logic to create or update. Let's just ensure the code
    //    can handle an existing record.
    let analysis_repo = PostgresUserAnalysisRepository::new(db.pool().clone());

    // 5) Call the main function that runs partition housekeeping + archiving + user analysis
    //    For real usage it’s called on an interval. For test, we call it directly:
    run_biweekly_maintenance(&db, &analysis_repo).await?;

    // 6) Confirm that older messages are gone from `chat_messages` and are in `chat_messages_archive`.
    //    Meanwhile, the new messages remain in `chat_messages`.
    let older_in_chat = sqlx::query(
        r#"
        SELECT message_id FROM chat_messages
        WHERE message_id LIKE 'old_msg_%'
        "#
    )
        .fetch_all(db.pool())
        .await?;
    assert!(
        older_in_chat.is_empty(),
        "Old messages should have been archived and removed from `chat_messages`"
    );

    let older_in_archive = sqlx::query(
        r#"
        SELECT message_id FROM chat_messages_archive
        WHERE message_id LIKE 'old_msg_%'
        "#
    )
        .fetch_all(db.pool())
        .await?;
    assert_eq!(older_in_archive.len(), 3, "All 3 old messages should be in the archive");

    // Newer messages should still be in chat_messages
    let newer_in_chat = sqlx::query(
        r#"
        SELECT message_id FROM chat_messages
        WHERE message_id LIKE 'new_msg_%'
        "#
    )
        .fetch_all(db.pool())
        .await?;
    assert_eq!(newer_in_chat.len(), 2, "The 2 newer messages should remain in chat_messages");

    // 7) Confirm user_analysis was created or updated
    //    We inserted user_1 who posted messages older + newer than 30 days.
    //    The code lumps "older than 30 days" into the archiving flow, then calls
    //    generate_user_summaries(...). Let's see if we got a user_analysis row:
    let found_analysis = analysis_repo.get_analysis(user_id).await?;
    assert!(
        found_analysis.is_some(),
        "UserAnalysis record should have been created or updated for user_1"
    );

    // 8) Done - test passes if we reached here without panic
    Ok(())
}

/// Example test focusing on partition creation/dropping (optional).
#[tokio::test]
async fn test_partition_maintenance() -> Result<(), Error> {
    let db = setup_test_database().await?;
    db.migrate().await?;

    // For demonstration: run partition maintenance with a 60‑day cutoff
    run_partition_maintenance(&db, 60).await?;

    // Optionally, check that a partition for "this month" was created:
    // We'll parse the current year/month from the code you are testing, or replicate it:
    let now = Utc::now();
    let year = now.year();
    let month = now.month();
    let partition_name = format!("chat_messages_{:04}{:02}", year, month);

    // See if that partition table exists:
    let partition_exists = sqlx::query(
        r#"
        SELECT EXISTS (
            SELECT FROM pg_tables
            WHERE tablename = $1
        )
        "#,
    )
        .bind(&partition_name)
        .fetch_one(db.pool())
        .await?;

    let partition_exists_bool: bool = partition_exists.try_get("exists")?;
    assert!(
        partition_exists_bool,
        "Partition for the current month should exist"
    );

    // We won't do a full test of dropping old partitions unless
    // you manually create them for older months, etc.

    Ok(())
}

/// Example test focusing on archive & user analysis (optional).
#[tokio::test]
async fn test_run_archive_and_analysis() -> Result<(), Error> {
    let db = setup_test_database().await?;
    db.migrate().await?;

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

    // Insert messages from 35 days ago => should be archived
    let older_ts = Utc::now() - Duration::days(35);
    for i in 0..5 {
        sqlx::query(
            r#"
            INSERT INTO chat_messages (message_id, platform, channel, user_id, message_text, timestamp)
            VALUES ($1, 'twitch', 'analysis_channel', $2, $3, $4)
            "#,
        )
            .bind(format!("old_analysis_msg_{}", i))
            .bind(user_id)
            .bind(format!("old message #{}", i))
            .bind(older_ts)
            .execute(db.pool())
            .await?;
    }

    // Insert messages from 5 days ago => remain
    let newer_ts = Utc::now() - Duration::days(5);
    for i in 0..3 {
        sqlx::query(
            r#"
            INSERT INTO chat_messages (message_id, platform, channel, user_id, message_text, timestamp)
            VALUES ($1, 'twitch', 'analysis_channel', $2, $3, $4)
            "#,
        )
            .bind(format!("new_analysis_msg_{}", i))
            .bind(user_id)
            .bind(format!("newer message #{}", i))
            .bind(newer_ts)
            .execute(db.pool())
            .await?;
    }

    // Run just the "archiving + user analysis" step
    let analysis_repo = PostgresUserAnalysisRepository::new(db.pool().clone());
    run_archive_and_analysis(&db, &analysis_repo).await?;

    // Check archived
    let old_in_main = sqlx::query(
        r#"
        SELECT message_id FROM chat_messages
        WHERE message_id LIKE 'old_analysis_msg_%'
        "#,
    )
        .fetch_all(db.pool())
        .await?;
    assert!(
        old_in_main.is_empty(),
        "Older messages should have been archived out of chat_messages"
    );

    let old_in_archive = sqlx::query(
        r#"
        SELECT message_id FROM chat_messages_archive
        WHERE message_id LIKE 'old_analysis_msg_%'
        "#,
    )
        .fetch_all(db.pool())
        .await?;
    assert_eq!(old_in_archive.len(), 5);

    // Check that the 3 newer remain
    let new_in_main = sqlx::query(
        r#"
        SELECT message_id FROM chat_messages
        WHERE message_id LIKE 'new_analysis_msg_%'
        "#,
    )
        .fetch_all(db.pool())
        .await?;
    assert_eq!(new_in_main.len(), 3);

    // Check user_analysis was updated or created
    let analysis = analysis_repo.get_analysis(user_id).await?;
    assert!(analysis.is_some(), "Should have a user_analysis for user");
    let an = analysis.unwrap();
    // The default logic in run_ai_scoring + merging is naive, but let's just confirm
    // it updated:
    assert!(an.spam_score > 0.0, "Should have some non-default spam_score from the sample code.");

    // Also check user_analysis_history for a new row:
    let year_month = format!("{}-{:02}", Utc::now().year(), Utc::now().month());
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

    assert_eq!(hist_rows.len(), 1, "Should have 1 monthly summary row in user_analysis_history");

    Ok(())
}