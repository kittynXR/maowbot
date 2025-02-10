// File: maowbot-core/src/tasks/biweekly_maintenance.rs

use std::sync::Arc;
use chrono::{DateTime, Utc, NaiveDate, Datelike};
use sqlx::{Pool, Postgres, Row};
use tracing::{info, error};
use tokio::time::sleep;
use std::time::Duration;
use sqlx::error::BoxDynError;
use uuid::Uuid;

use crate::db::Database;
use crate::repositories::postgres::user_analysis::{PostgresUserAnalysisRepository, UserAnalysisRepository};
use crate::repositories::postgres::analytics::ChatMessage;
use crate::models::UserAnalysis;
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

    // 1) Create partitions
    run_partition_maintenance(db).await?;
    info!("Partition creation done...");

    // 2) User analysis
    run_analysis(db, user_analysis_repo).await?;
    info!("Analysis done...");

    info!("Biweekly maintenance is complete.");
    Ok(())
}

/// Creates partitions for the current and next month.
/// (Does NOT drop or clean old partitions.)
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

    create_month_partition_if_needed(pool, this_month_first).await?;
    create_month_partition_if_needed(pool, next_month).await?;

    Ok(())
}

/// Creates a new partition for the given month's first day if it doesn't already exist.
async fn create_month_partition_if_needed(
    pool: &Pool<Postgres>,
    first_day: NaiveDate
) -> Result<(), Error> {
    let year = first_day.year();
    let month = first_day.month();
    let partition_name = format!("chat_messages_{:04}{:02}", year, month);

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

    let range_start = first_day
        .and_hms_opt(0, 0, 0)
        .ok_or_else(|| {
            Error::Database(sqlx::Error::Configuration(
                BoxDynError::from("Invalid partition start timestamp.".to_string()),
            ))
        })?;

    let range_end = next_month
        .and_hms_opt(0, 0, 0)
        .ok_or_else(|| {
            Error::Database(sqlx::Error::Configuration(
                BoxDynError::from("Invalid partition end timestamp.".to_string()),
            ))
        })?;

    let create_sql = format!(
        r#"
        CREATE TABLE IF NOT EXISTS {partition}
        PARTITION OF chat_messages
        FOR VALUES FROM ('{start}') TO ('{end}');
        "#,
        partition = partition_name,
        start = range_start,
        end = range_end
    );

    sqlx::query(&create_sql).execute(pool).await?;
    info!("Ensured partition: {}", partition_name);
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
        let user_id: String = row.try_get("user_id")?;

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
        let hist_id = Uuid::new_v4().to_string();
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
        if let Some(mut analysis) = user_analysis_repo.get_analysis(&user_id).await? {
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
                user_analysis_id: Uuid::new_v4().to_string(),
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