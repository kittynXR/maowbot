// File: maowbot-core/tests/test_biweekly_maintenance.rs

use chrono::{Datelike, Utc};
use sqlx::{Row};
use uuid::Uuid;

use maowbot_core::{
    db::Database,
    models::UserAnalysis,
    repositories::postgres::analytics::ChatMessage,
    repositories::postgres::user_analysis::PostgresUserAnalysisRepository,
    tasks::biweekly_maintenance::{run_partition_maintenance, run_analysis, run_biweekly_maintenance},
};
use maowbot_core::test_utils::helpers::{setup_test_database, clean_database};

#[tokio::test]
async fn test_partition_creation_only() -> Result<(), maowbot_core::Error> {
    // 1) Setup a fresh test DB
    let db: Database = setup_test_database().await?;

    // 2) Call partition creation
    run_partition_maintenance(&db).await?;

    // 3) Verify that partition tables exist (e.g. current & next month)
    //    We can check by querying pg_class or pg_inherits for the new partition name(s).
    let now = Utc::now();
    let current_year = now.year();
    let current_month = now.month();
    let partition_name = format!("chat_messages_{:04}{:02}", current_year, current_month);

    let row_current = sqlx::query("SELECT 1 FROM pg_class WHERE relname = $1")
        .bind(&partition_name)
        .fetch_optional(db.pool())
        .await?;

    assert!(
        row_current.is_some(),
        "Expected current partition '{}' to be created.",
        partition_name
    );

    // Next month check
    let next_month_year = if current_month == 12 {
        current_year + 1
    } else {
        current_year
    };
    let next_month = if current_month == 12 { 1 } else { current_month + 1 };
    let next_partition = format!("chat_messages_{:04}{:02}", next_month_year, next_month);

    let row_next = sqlx::query("SELECT 1 FROM pg_class WHERE relname = $1")
        .bind(&next_partition)
        .fetch_optional(db.pool())
        .await?;

    assert!(
        row_next.is_some(),
        "Expected next partition '{}' to be created.",
        next_partition
    );

    Ok(())
}

#[tokio::test]
async fn test_user_analysis_when_no_messages() -> Result<(), maowbot_core::Error> {
    // 1) Setup fresh DB
    let db = setup_test_database().await?;
    let analysis_repo = PostgresUserAnalysisRepository::new(db.pool().clone());

    // 2) No chat_messages inserted -> run analysis
    run_analysis(&db, &analysis_repo).await?;

    // 3) We expect no user_analysis or user_analysis_history records
    let count_analysis: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM user_analysis")
        .fetch_one(db.pool())
        .await?;
    assert_eq!(count_analysis.0, 0, "No user_analysis rows expected if no messages.");

    let count_history: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM user_analysis_history")
        .fetch_one(db.pool())
        .await?;
    assert_eq!(count_history.0, 0, "No user_analysis_history rows expected if no messages.");

    Ok(())
}

#[tokio::test]
async fn test_user_analysis_with_messages() -> Result<(), maowbot_core::Error> {
    // 1) Setup
    let db = setup_test_database().await?;
    let pool = db.pool();
    let analysis_repo = PostgresUserAnalysisRepository::new(pool.clone());

    // 2) Create the user first in `users` so that chat_messages can reference it.
    sqlx::query(r#"
        INSERT INTO users (
            user_id,
            created_at,
            last_seen,
            is_active,
            global_username
        ) VALUES ($1, $2, $3, $4, $5)
    "#)
        .bind("user123")
        .bind(Utc::now())
        .bind(Utc::now())
        .bind(true)
        .bind("user123")
        .execute(pool)
        .await?;

    // 3) Insert some chat_messages referencing that user
    let now = Utc::now();
    let chat_inserts = vec![
        ChatMessage {
            message_id: Uuid::new_v4().to_string(),
            platform: "test_platform".to_string(),
            channel: "test_channel".to_string(),
            user_id: "user123".to_string(),
            message_text: "Hello world".to_string(),
            timestamp: now,
            metadata: None,
        },
        ChatMessage {
            message_id: Uuid::new_v4().to_string(),
            platform: "test_platform".to_string(),
            channel: "test_channel".to_string(),
            user_id: "user123".to_string(),
            message_text: "Another message".to_string(),
            timestamp: now,
            metadata: None,
        },
    ];

    for msg in &chat_inserts {
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
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            "#,
        )
            .bind(&msg.message_id)
            .bind(&msg.platform)
            .bind(&msg.channel)
            .bind(&msg.user_id)
            .bind(&msg.message_text)
            .bind(msg.timestamp)
            .bind(None::<String>)
            .execute(pool)
            .await?;
    }

    // 4) Run analysis
    run_analysis(&db, &analysis_repo).await?;

    // 5) Verify user_analysis and user_analysis_history
    let row = sqlx::query_as::<_, (String, f32, f32, f32, f32, Option<String>)>(
        r#"
        SELECT user_id, spam_score, intelligibility_score,
               quality_score, horni_score, ai_notes
        FROM user_analysis
        WHERE user_id = $1
        "#,
    )
        .bind("user123")
        .fetch_one(pool)
        .await?;

    let (user_id, spam, intel, quality, horni, notes) = row;
    assert_eq!(user_id, "user123");
    // Check approximate values from dummy run_ai_scoring
    assert!(spam >= 0.0);
    assert_eq!(intel, 0.5);
    assert_eq!(quality, 0.6);
    assert_eq!(horni, 0.2);
    assert!(notes.is_some(), "Expected AI notes to be populated");

    let count_history: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM user_analysis_history WHERE user_id = $1"
    )
        .bind("user123")
        .fetch_one(pool)
        .await?;
    assert_eq!(count_history.0, 1, "Should have exactly one history entry");

    Ok(())
}

#[tokio::test]
async fn test_run_biweekly_maintenance_end_to_end() -> Result<(), maowbot_core::Error> {
    // 1) Setup
    let db = setup_test_database().await?;
    let pool = db.pool();
    let analysis_repo = PostgresUserAnalysisRepository::new(pool.clone());

    // 2) Create a user for "bob123"
    sqlx::query(r#"
        INSERT INTO users (
            user_id,
            created_at,
            last_seen,
            is_active,
            global_username
        ) VALUES ($1, $2, $3, $4, $5)
    "#)
        .bind("bob123")
        .bind(Utc::now())
        .bind(Utc::now())
        .bind(true)
        .bind("bob123")
        .execute(pool)
        .await?;

    // 3) Insert a single message referencing bob123
    let msg = ChatMessage {
        message_id: Uuid::new_v4().to_string(),
        platform: "test_platform".to_string(),
        channel: "test_channel".to_string(),
        user_id: "bob123".to_string(),
        message_text: "Testing integrated function".to_string(),
        timestamp: Utc::now(),
        metadata: None,
    };
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
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        "#,
    )
        .bind(&msg.message_id)
        .bind(&msg.platform)
        .bind(&msg.channel)
        .bind(&msg.user_id)
        .bind(&msg.message_text)
        .bind(msg.timestamp)
        .bind(None::<String>)
        .execute(pool)
        .await?;

    // 4) Run the full maintenance
    run_biweekly_maintenance(&db, &analysis_repo).await?;

    // 5) Check that partitions exist
    let now = Utc::now();
    let current_year = now.year();
    let current_month = now.month();
    let partition_name = format!("chat_messages_{:04}{:02}", current_year, current_month);

    let row_current = sqlx::query("SELECT 1 FROM pg_class WHERE relname = $1")
        .bind(partition_name.clone())
        .fetch_optional(pool)
        .await?;
    assert!(
        row_current.is_some(),
        "Expected partition '{}' from run_biweekly_maintenance",
        partition_name
    );

    // 6) Verify user_analysis
    let bob_analysis = sqlx::query(
        "SELECT 1 FROM user_analysis WHERE user_id = $1"
    )
        .bind("bob123")
        .fetch_optional(pool)
        .await?;
    assert!(
        bob_analysis.is_some(),
        "Expected user_analysis row for bob123"
    );

    Ok(())
}