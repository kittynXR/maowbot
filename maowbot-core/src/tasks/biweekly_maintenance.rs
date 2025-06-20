// File: maowbot-core/src/tasks/biweekly_maintenance.rs

use std::sync::Arc;
use chrono::{DateTime, Utc, NaiveDate, Datelike};
use sqlx::{Pool, Postgres, Row};
use tracing::{info, error, debug};
use std::time::Duration;
use sqlx::error::BoxDynError;
use uuid::Uuid;
use maowbot_common::models::UserAnalysis;
use crate::db::Database;
use crate::repositories::postgres::user_analysis::{PostgresUserAnalysisRepository, UserAnalysisRepository};
use crate::repositories::postgres::analytics::ChatMessage;
use crate::Error;
use crate::eventbus::EventBus;

pub fn spawn_biweekly_maintenance_task(
    db: Database,
    user_analysis_repo: PostgresUserAnalysisRepository,
    event_bus: Arc<EventBus>,  // <--- pass in
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(14 * 24 * 3600));
        // Clone the watch channel so we can break out when shutdown is signaled
        let mut shutdown_rx = event_bus.shutdown_rx.clone();

        loop {
            tokio::select! {
                _ = interval.tick() => {
                    if let Err(e) = run_biweekly_maintenance(&db, &user_analysis_repo).await {
                        error!("Biweekly maintenance failed: {:?}", e);
                    }
                },
                Ok(_) = shutdown_rx.changed() => {
                    if *shutdown_rx.borrow() {
                        info!("Biweekly maintenance: shutting down cleanly.");
                        break;
                    }
                }
            }
        }

        info!("Biweekly maintenance task exited.");
    })
}

/// Runs biweekly maintenance: creates partitions for current + next month, then user analysis.
pub async fn run_biweekly_maintenance(
    db: &Database,
    user_analysis_repo: &PostgresUserAnalysisRepository,
) -> Result<(), Error> {
    info!("Starting biweekly maintenance tasks...");

    // 1) Create partitions for all partitioned tables
    run_partition_maintenance(db).await?;
    info!("Partition creation done...");

    // 2) Drop old partitions based on retention policies
    run_partition_cleanup(db).await?;
    info!("Partition cleanup done...");

    // 3) User analysis
    run_analysis(db, user_analysis_repo).await?;
    info!("Analysis done...");

    info!("Biweekly maintenance is complete.");
    Ok(())
}

/// Creates partitions for all partitioned tables for current and next month.
pub async fn run_partition_maintenance(db: &Database) -> Result<(), Error> {
    info!("Running partition maintenance (creation only)...");
    let pool = db.pool();

    let now = Utc::now().naive_utc().date();
    let (year, month) = (now.year(), now.month());

    let this_month_first = NaiveDate::from_ymd_opt(year, month, 1)
        .ok_or_else(|| {
            Error::Database(sqlx::Error::Configuration(
                BoxDynError::from("Invalid date for current month partition.".to_string()),
            ))
        })?;

    let next_month = if month == 12 {
        NaiveDate::from_ymd_opt(year + 1, 1, 1)
            .ok_or_else(|| {
                Error::Database(sqlx::Error::Configuration(
                    BoxDynError::from("Invalid date for next month partition.".to_string()),
                ))
            })?
    } else {
        NaiveDate::from_ymd_opt(year, month + 1, 1)
            .ok_or_else(|| {
                Error::Database(sqlx::Error::Configuration(
                    BoxDynError::from("Invalid date for next month partition.".to_string()),
                ))
            })?
    };

    // Create partitions for all partitioned tables
    let partitioned_tables = vec![
        ("chat_messages", "timestamp"),
        ("analytics_events", "created_at"),
        ("command_usage", "executed_at"),
        ("redeem_usage", "redeemed_at"),
        ("pipeline_execution_log", "started_at"),
    ];

    for (table_name, time_column) in partitioned_tables {
        match create_month_partition_if_needed(pool, table_name, time_column, this_month_first).await {
            Ok(_) => {},
            Err(e) => {
                error!("Failed to create partition for {} (current month): {:?}", table_name, e);
                // Continue with other tables even if one fails
            }
        }
        
        match create_month_partition_if_needed(pool, table_name, time_column, next_month).await {
            Ok(_) => {},
            Err(e) => {
                error!("Failed to create partition for {} (next month): {:?}", table_name, e);
                // Continue with other tables even if one fails
            }
        }
    }

    Ok(())
}

/// Creates a new partition for the given month's first day if it doesn't already exist.
async fn create_month_partition_if_needed(
    pool: &Pool<Postgres>,
    table_name: &str,
    time_column: &str,
    first_day: NaiveDate
) -> Result<(), Error> {
    let year = first_day.year();
    let month = first_day.month();
    let partition_name = format!("{}_{:04}{:02}", table_name, year, month);

    let next_month = if month == 12 {
        NaiveDate::from_ymd_opt(year + 1, 1, 1)
            .ok_or_else(|| {
                Error::Database(sqlx::Error::Configuration(
                    BoxDynError::from("Invalid date for next month partition.".to_string()),
                ))
            })?
    } else {
        NaiveDate::from_ymd_opt(year, month + 1, 1)
            .ok_or_else(|| {
                Error::Database(sqlx::Error::Configuration(
                    BoxDynError::from("Invalid date for next month partition.".to_string()),
                ))
            })?
    };

    // Convert NaiveDate to DateTime<Utc> for timezone-aware timestamps
    let range_start = first_day
        .and_hms_opt(0, 0, 0)
        .ok_or_else(|| {
            Error::Database(sqlx::Error::Configuration(
                BoxDynError::from("Invalid partition start timestamp.".to_string()),
            ))
        })?
        .and_utc();

    let range_end = next_month
        .and_hms_opt(0, 0, 0)
        .ok_or_else(|| {
            Error::Database(sqlx::Error::Configuration(
                BoxDynError::from("Invalid partition end timestamp.".to_string()),
            ))
        })?
        .and_utc();

    // Format timestamps as UTC to ensure partitions work regardless of server timezone
    let start_str = range_start.format("%Y-%m-%d %H:%M:%S").to_string();
    let end_str = range_end.format("%Y-%m-%d %H:%M:%S").to_string();
    
    let create_sql = format!(
        r#"
        CREATE TABLE IF NOT EXISTS {partition}
        PARTITION OF {parent_table}
        FOR VALUES FROM (TIMESTAMP '{start}' AT TIME ZONE 'UTC') 
                      TO (TIMESTAMP '{end}' AT TIME ZONE 'UTC');
        "#,
        partition = partition_name,
        parent_table = table_name,
        start = start_str,
        end = end_str
    );

    sqlx::query(&create_sql).execute(pool).await?;
    info!("Ensured partition: {}", partition_name);
    Ok(())
}

/// Drops old partitions based on retention policies
pub async fn run_partition_cleanup(db: &Database) -> Result<(), Error> {
    info!("Running partition cleanup...");
    let pool = db.pool();
    
    // Get default retention days from config
    let default_retention: i64 = sqlx::query_scalar(
        "SELECT COALESCE(config_value::bigint, 30) FROM bot_config WHERE config_key = 'chat_logging.default_retention_days'"
    )
    .fetch_optional(pool)
    .await?
    .unwrap_or(30);
    
    // Get all partitioned tables and their retention policies
    let retention_configs: Vec<(String, i64)> = vec![
        ("chat_messages".to_string(), default_retention),
        ("analytics_events".to_string(), 90), // Keep analytics for 3 months
        ("command_usage".to_string(), 30),
        ("redeem_usage".to_string(), 30),
        ("pipeline_execution_log".to_string(), 7), // Only keep pipeline logs for 7 days
    ];
    
    let now = Utc::now();
    
    for (base_table, retention_days) in retention_configs {
        let cutoff_date = now - chrono::Duration::days(retention_days);
        
        // Find partitions older than retention period
        let old_partitions: Vec<String> = sqlx::query_scalar(
            r#"
            SELECT tablename 
            FROM pg_tables 
            WHERE schemaname = 'public' 
            AND tablename LIKE $1
            AND tablename ~ '[0-9]{6}$'
            ORDER BY tablename
            "#
        )
        .bind(format!("{}_%", base_table))
        .fetch_all(pool)
        .await?;
        
        for partition in old_partitions {
            // Extract year and month from partition name (format: tablename_YYYYMM)
            if let Some(date_part) = partition.split('_').last() {
                if date_part.len() == 6 {
                    if let (Ok(year), Ok(month)) = (
                        date_part[0..4].parse::<i32>(),
                        date_part[4..6].parse::<u32>()
                    ) {
                        let partition_start = NaiveDate::from_ymd_opt(year, month, 1);
                        if let Some(partition_start_date) = partition_start {
                            // Calculate the last day of the partition's month
                            let next_month = if month == 12 {
                                NaiveDate::from_ymd_opt(year + 1, 1, 1)
                            } else {
                                NaiveDate::from_ymd_opt(year, month + 1, 1)
                            };
                            
                            if let Some(partition_end) = next_month {
                                // Only drop if the entire partition is older than cutoff
                                // AND the partition doesn't cover the current date
                                let current_date = now.naive_utc().date();
                                
                                if partition_end <= cutoff_date.naive_utc().date() && partition_end <= current_date {
                                    // Check if pre-drop pipeline should be executed
                                    if base_table == "chat_messages" {
                                        // TODO: Execute pre-drop pipeline if configured
                                        // This would process/archive the data before dropping
                                    }
                                    
                                    // Drop the old partition
                                    match sqlx::query(&format!("DROP TABLE IF EXISTS {} CASCADE", partition))
                                        .execute(pool)
                                        .await
                                    {
                                        Ok(_) => info!("Dropped old partition: {}", partition),
                                        Err(e) => error!("Failed to drop partition {}: {:?}", partition, e),
                                    }
                                } else {
                                    debug!("Keeping partition {} - covers current date or within retention", partition);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    
    Ok(())
}

/// Runs user analysis (aggregates last 30 days of chat_messages).
pub async fn run_analysis(
    db: &Database,
    user_analysis_repo: &PostgresUserAnalysisRepository,
) -> Result<(), Error> {
    info!("Running user analysis...");
    let start_ts = Utc::now() - chrono::Duration::days(30);
    let end_ts   = Utc::now();
    generate_user_summaries(db, user_analysis_repo, start_ts, end_ts).await
}

/// Gathers each user's messages in [start_ts, end_ts), runs AI scoring,
/// and updates `user_analysis` + `user_analysis_history`.
async fn generate_user_summaries(
    db: &Database,
    user_analysis_repo: &PostgresUserAnalysisRepository,
    start_ts: DateTime<Utc>,
    end_ts: DateTime<Utc>,
) -> Result<(), Error> {
    info!("Generating user summaries from {} to {}", start_ts, end_ts);

    // 1) Distinct user_id in that time range
    let user_rows = sqlx::query(
        r#"
        SELECT DISTINCT user_id
        FROM chat_messages
        WHERE timestamp >= $1 AND timestamp < $2
        "#,
    )
        .bind(start_ts)
        .bind(end_ts)
        .fetch_all(db.pool())
        .await?;

    // 2) For each user, gather messages & compute analysis
    for row in user_rows {
        let user_id: Uuid = row.try_get("user_id")?;

        let messages = sqlx::query_as::<_, ChatMessage>(
            r#"
            SELECT message_id, platform, channel, user_id, message_text, timestamp, metadata
            FROM chat_messages
            WHERE user_id = $1 AND timestamp >= $2 AND timestamp < $3
            "#,
        )
            .bind(&user_id)
            .bind(start_ts)
            .bind(end_ts)
            .fetch_all(db.pool())
            .await?;

        let (spam, intel, quality, horni, summary) = run_ai_scoring(&messages).await;

        // Insert a user_analysis_history entry
        let hist_id = Uuid::new_v4();
        let year_month = format!("{}-{:02}", Utc::now().year(), Utc::now().month());
        sqlx::query(
            r#"
            INSERT INTO user_analysis_history (
                user_analysis_history_id,
                user_id,
                year_month,
                spam_score,
                intelligibility_score,
                quality_score,
                horni_score,
                ai_notes,
                created_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            "#,
        )
            .bind(hist_id)
            .bind(&user_id)
            .bind(&year_month)
            .bind(spam)
            .bind(intel)
            .bind(quality)
            .bind(horni)
            .bind(&summary)
            .bind(Utc::now())
            .execute(db.pool())
            .await?;

        // Update or create user_analysis
        if let Some(mut analysis) = user_analysis_repo.get_analysis(user_id).await? {
            // Simple weighted approach
            analysis.spam_score = 0.7 * analysis.spam_score + 0.3 * spam;
            analysis.intelligibility_score = 0.7 * analysis.intelligibility_score + 0.3 * intel;
            analysis.quality_score = 0.7 * analysis.quality_score + 0.3 * quality;
            analysis.horni_score = 0.7 * analysis.horni_score + 0.3 * horni;

            let old_notes = analysis.ai_notes.clone().unwrap_or_default();
            let appended_notes = format!("{}\n\n=== {} summary ===\n{}", old_notes, year_month, summary);
            analysis.ai_notes = Some(appended_notes);

            user_analysis_repo.update_analysis(&analysis).await?;
        } else {
            // If there's no existing analysis, create a brand-new one:
            let new_one = UserAnalysis {
                user_analysis_id: Uuid::new_v4(),
                user_id: user_id.clone(),
                spam_score: spam,
                intelligibility_score: intel,
                quality_score: quality,
                horni_score: horni,
                ai_notes: Some(summary),
                moderator_notes: None,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            };
            user_analysis_repo.create_analysis(&new_one).await?;
        }
    }

    Ok(())
}

/// Dummy AI scoring function just for illustration.
async fn run_ai_scoring(messages: &[ChatMessage]) -> (f32, f32, f32, f32, String) {
    let count = messages.len() as f32;
    // Example logic
    let spam = 0.1 * count.min(5.0);
    let intel = 0.5;
    let quality = 0.6;
    let horni = 0.2;

    let summary = format!("User posted {} messages. Spam estimate: {:.2}", count, spam);
    (spam, intel, quality, horni, summary)
}