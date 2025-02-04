// File: maowbot-core/src/tasks/biweekly_maintenance.rs

use chrono::{DateTime, Utc, NaiveDate, Datelike};
use sqlx::{Pool, Postgres, Row};
use tracing::{info, error};
use tokio::time::sleep;
use std::time::Duration;
use uuid::Uuid;

use crate::db::Database;
use crate::repositories::postgres::user_analysis::{PostgresUserAnalysisRepository, UserAnalysisRepository};
use crate::repositories::postgres::analytics::ChatMessage;
use crate::models::UserAnalysis;
use crate::Error;

/// Spawns a background task that runs every two weeks, performing partition housekeeping and user analysis.
pub fn spawn_biweekly_maintenance_task(
    db: Database,
    user_analysis_repo: PostgresUserAnalysisRepository,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(14 * 24 * 3600));
        loop {
            interval.tick().await;
            if let Err(e) = run_biweekly_maintenance(&db, &user_analysis_repo).await {
                error!("Biweekly maintenance failed: {:?}", e);
            }
        }
    })
}

/// Runs biweekly maintenance: first housekeeping (partition drop or cleaning), then user analysis.
pub async fn run_biweekly_maintenance(
    db: &Database,
    user_analysis_repo: &PostgresUserAnalysisRepository,
) -> Result<(), Error> {
    info!("Starting biweekly maintenance tasks...");

    // 1) Partition housekeeping (drop explicit partitions or clean default partition)
    run_partition_maintenance(db, 60).await?;
    info!("Partition maintenance done. Sleeping 1s...");
    sleep(Duration::from_secs(1)).await;

    // 2) Run user analysis for the last 30 days.
    run_analysis(db, user_analysis_repo).await?;
    info!("Analysis done. Sleeping 1s...");
    sleep(Duration::from_secs(1)).await;

    info!("Biweekly maintenance is complete.");
    Ok(())
}

/// Performs partition maintenance: creates partitions for the current and next month,
/// then drops explicit partitions older than cutoff or cleans default partitions.
pub async fn run_partition_maintenance(db: &Database, cutoff_days: i64) -> Result<(), Error> {
    info!("Running partition maintenance with cutoff_days = {}...", cutoff_days);
    let pool = db.pool();

    // Create explicit partitions for the current month and next month.
    let now = Utc::now().naive_utc().date();
    let (year, month) = (now.year(), now.month());
    let this_month_first = NaiveDate::from_ymd_opt(year, month, 1).unwrap();
    let next_month = if month == 12 {
        NaiveDate::from_ymd_opt(year + 1, 1, 1).unwrap()
    } else {
        NaiveDate::from_ymd_opt(year, month + 1, 1).unwrap()
    };

    create_month_partition_if_needed(pool, this_month_first).await?;
    create_month_partition_if_needed(pool, next_month).await?;

    // Drop partitions (or clean default partitions) older than cutoff.
    drop_old_chat_partitions(pool, cutoff_days).await?;

    Ok(())
}

/// Creates a partition for a given month if it does not already exist.
async fn create_month_partition_if_needed(pool: &Pool<Postgres>, first_day: NaiveDate) -> Result<(), Error> {
    let year = first_day.year();
    let month = first_day.month();
    let partition_name = format!("chat_messages_{:04}{:02}", year, month);

    let next_month = if month == 12 {
        NaiveDate::from_ymd_opt(year + 1, 1, 1).unwrap()
    } else {
        NaiveDate::from_ymd_opt(year, month + 1, 1).unwrap()
    };

    let range_start = first_day.and_hms_opt(0, 0, 0).unwrap();
    let range_end   = next_month.and_hms_opt(0, 0, 0).unwrap();

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

/// Drops explicit partitions whose upper bound is older than cutoff or cleans default partitions.
async fn drop_old_chat_partitions(pool: &Pool<Postgres>, cutoff_days: i64) -> Result<(), Error> {
    let now = Utc::now();
    let cutoff = now - chrono::Duration::days(cutoff_days);

    let child_partitions_sql = r#"
        SELECT (inhrelid::regclass)::text AS partition_name
        FROM pg_inherits
        WHERE inhparent::regclass = 'chat_messages'::regclass;
    "#;

    let rows = sqlx::query(child_partitions_sql).fetch_all(pool).await?;

    for row in rows {
        let partition_name: String = row.get("partition_name");
        let boundary_sql = format!(
            r#"
            SELECT pg_get_expr(relpartbound, oid) AS boundary
            FROM pg_class
            WHERE relname = '{partition_name}'
            "#,
            partition_name = partition_name
        );
        let boundary_row = sqlx::query_as::<_, (Option<String>,)>(&boundary_sql)
            .fetch_one(pool)
            .await?;

        // If the boundary expression is missing or empty, treat as default partition.
        let boundary_expr = boundary_row.0.unwrap_or_default();
        if boundary_expr.trim().is_empty() {
            // This is a default partition: delete rows older than cutoff.
            info!("Attempting to delete rows older than {} in default partition: {}", cutoff, partition_name);

            let delete_sql = format!("DELETE FROM {} WHERE timestamp < $1;", partition_name);
            let res = sqlx::query(&delete_sql).bind(cutoff).execute(pool).await?;
            info!(
                "Cleaned {} rows from default partition {}",
                res.rows_affected(),
                partition_name
            );
        } else {
            // This partition has an explicit boundary.
            if let Some(ts_str) = extract_upper_bound_ts(&boundary_expr) {
                if let Ok(bound_dt) = ts_str.parse::<DateTime<Utc>>() {
                    if bound_dt < cutoff {
                        let drop_sql = format!("DROP TABLE IF EXISTS {};", partition_name);
                        sqlx::query(&drop_sql).execute(pool).await?;
                        info!("Dropped old partition {}", partition_name);
                    }
                }
            }
        }
    }

    Ok(())
}

/// Extracts the upper bound timestamp from a boundary expression string.
/// For example, given:
///   "FOR VALUES FROM ('2025-01-01 00:00:00') TO ('2025-02-01 00:00:00')"
/// it returns Some("2025-02-01 00:00:00").
fn extract_upper_bound_ts(expr_text: &str) -> Option<String> {
    let lower = expr_text.to_lowercase();
    if let Some(idx) = lower.find(" to ('") {
        let part = &expr_text[(idx + 5)..];
        if let Some(end_paren) = part.find(')') {
            return Some(part[0..end_paren].trim().to_string());
        }
    }
    None
}

/// Runs user analysis: aggregates chat messages from the past 30 days and updates analysis records.
pub async fn run_analysis(
    db: &Database,
    user_analysis_repo: &PostgresUserAnalysisRepository,
) -> Result<(), Error> {
    info!("Running user analysis...");
    let start_ts = Utc::now() - chrono::Duration::days(30);
    let end_ts   = Utc::now();
    generate_user_summaries(db, user_analysis_repo, start_ts, end_ts).await
}

/// Aggregates user chat messages and updates user_analysis and user_analysis_history.
async fn generate_user_summaries(
    db: &Database,
    user_analysis_repo: &PostgresUserAnalysisRepository,
    start_ts: DateTime<Utc>,
    end_ts: DateTime<Utc>,
) -> Result<(), Error> {
    info!("Generating user summaries from {} to {}", start_ts, end_ts);

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

        let hist_id = Uuid::new_v4().to_string();
        let year_month = format!("{}-{:02}", Utc::now().year(), Utc::now().month());
        sqlx::query(
            r#"
            INSERT INTO user_analysis_history (
                user_analysis_history_id, user_id, year_month, spam_score,
                intelligibility_score, quality_score, horni_score, ai_notes, created_at
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

        if let Some(mut analysis) = user_analysis_repo.get_analysis(&user_id).await? {
            analysis.spam_score = 0.7 * analysis.spam_score + 0.3 * spam;
            analysis.intelligibility_score = 0.7 * analysis.intelligibility_score + 0.3 * intel;
            analysis.quality_score = 0.7 * analysis.quality_score + 0.3 * quality;
            analysis.horni_score = 0.7 * analysis.horni_score + 0.3 * horni;
            let old_notes = analysis.ai_notes.clone().unwrap_or_default();
            let appended_notes = format!("{}\n\n=== {} summary ===\n{}", old_notes, year_month, summary);
            analysis.ai_notes = Some(appended_notes);

            user_analysis_repo.update_analysis(&analysis).await?;
        } else {
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

/// Dummy AI scoring function.
async fn run_ai_scoring(messages: &[ChatMessage]) -> (f32, f32, f32, f32, String) {
    let count = messages.len() as f32;
    let spam = 0.1 * count.min(5.0);
    let intel = 0.5;
    let quality = 0.6;
    let horni = 0.2;
    let summary = format!("User posted {} messages. Spam estimate: {:.2}", count, spam);
    (spam, intel, quality, horni, summary)
}