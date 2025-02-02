// File: src/tasks/monthly_maintenance.rs
//
// This version uses Postgres and a second table (e.g. "chat_messages_archive")
// in the same database to store archived records, rather than SQLite attach/detach logic.

use std::path::PathBuf;
use chrono::{Datelike, NaiveDate, NaiveDateTime, Utc};
use uuid::Uuid;
use tracing::info;
use sqlx::{Row};

use crate::{
    Error,
    Database,
    // We'll reference your user analysis and chat_messages structures from the Postgres modules
    repositories::postgres::analytics::ChatMessage,
    repositories::postgres::user_analysis::{PostgresUserAnalysisRepository, UserAnalysisRepository},
    models::UserAnalysis,
};
use crate::utils::time::{to_epoch, from_epoch};

/// The main function you’ll call (e.g. from run_server) to check for months
/// that need archiving and run monthly summarization.
pub async fn maybe_run_monthly_maintenance(
    db: &Database,
    user_analysis_repo: &PostgresUserAnalysisRepository,
) -> Result<(), Error> {
    // Example: read last archived month, see if we need to do "2025-01"
    // We'll just hard-code a demo example here:
    let (start_ts, end_ts) = ("2025-01-01 00:00:00", "2025-02-01 00:00:00");

    // 1) Archive old chat messages from that range
    archive_one_month(db, start_ts, end_ts).await?;

    // 2) Summarize or do your user analysis in that monthly range
    generate_monthly_user_summaries(db, user_analysis_repo, start_ts, end_ts, "2025-01").await?;

    // 3) Update maintenance_state => "archived_until"= "2025-01"
    sqlx::query(r#"
        INSERT INTO maintenance_state (state_key, state_value)
        VALUES ('archived_until', $1)
        ON CONFLICT (state_key) DO UPDATE
            SET state_value = EXCLUDED.state_value
    "#)
        .bind("2025-01")
        .execute(db.pool())
        .await?;

    Ok(())
}

/// Reads the "archived_until" state from maintenance_state.
/// Returns e.g. Some("2024-12") or None if not set.
async fn read_last_archived_month(db: &Database) -> Result<Option<String>, Error> {
    let row = sqlx::query(
        r#"
        SELECT state_value
        FROM maintenance_state
        WHERE state_key = 'archived_until'
        "#
    )
        .fetch_optional(db.pool())
        .await?;

    if let Some(r) = row {
        Ok(Some(r.try_get("state_value")?))
    } else {
        Ok(None)
    }
}

/// Writes to maintenance_state: "archived_until" => e.g. "YYYY-MM".
async fn update_last_archived_month(db: &Database, year_month: &str) -> Result<(), Error> {
    sqlx::query(r#"
        INSERT INTO maintenance_state (state_key, state_value)
        VALUES ('archived_until', $1)
        ON CONFLICT (state_key) DO UPDATE
            SET state_value = EXCLUDED.state_value
    "#)
        .bind(year_month)
        .execute(db.pool())
        .await?;
    Ok(())
}

/// Collect missing months between `last_archived` and `target`.
pub fn collect_missing_months(
    last_archived: Option<&str>,
    target: &str
) -> Result<Vec<String>, Error> {
    if last_archived.is_none() {
        return Ok(vec![target.to_string()]);
    }

    let (la_y, la_m) = parse_year_month(last_archived.unwrap())?;
    let (tg_y, tg_m) = parse_year_month(target)?;

    let mut results = Vec::new();
    let (mut cy, mut cm) = next_month(la_y, la_m);
    while (cy < tg_y) || (cy == tg_y && cm <= tg_m) {
        results.push(format!("{:04}-{:02}", cy, cm));
        let (ny, nm) = next_month(cy, cm);
        cy = ny;
        cm = nm;
    }

    Ok(results)
}

/// Archive data for one month by copying from `chat_messages` to `chat_messages_archive`,
/// then deleting from `chat_messages`.
pub async fn archive_one_month(
    db: &Database,
    start_ts: &str,
    end_ts: &str,
) -> Result<(), Error> {
    // Convert timestamps to epoch seconds
    let start_dt = NaiveDateTime::parse_from_str(start_ts, "%Y-%m-%d %H:%M:%S")
        .map_err(|e| Error::Parse(e.to_string()))?;
    let end_dt = NaiveDateTime::parse_from_str(end_ts, "%Y-%m-%d %H:%M:%S")
        .map_err(|e| Error::Parse(e.to_string()))?;
    let start_epoch = to_epoch(start_dt);
    let end_epoch = to_epoch(end_dt);

    // 1) Insert into `chat_messages_archive` from the main table
    //    (You'd presumably create chat_messages_archive in a prior migration.)
    sqlx::query(r#"
        INSERT INTO chat_messages_archive
        (message_id, platform, channel, user_id, message_text, timestamp, metadata)
        SELECT message_id, platform, channel, user_id, message_text, timestamp, metadata
        FROM chat_messages
        WHERE timestamp >= $1
          AND timestamp < $2
    "#)
        .bind(start_epoch)
        .bind(end_epoch)
        .execute(db.pool())
        .await?;

    // 2) Delete them from the main table
    sqlx::query(r#"
        DELETE FROM chat_messages
        WHERE timestamp >= $1
          AND timestamp < $2
    "#)
        .bind(start_epoch)
        .bind(end_epoch)
        .execute(db.pool())
        .await?;

    Ok(())
}

/// Summarizes chat messages from [start_ts, end_ts) => store in user_analysis_history + update user_analysis.
async fn generate_monthly_user_summaries(
    db: &Database,
    user_analysis_repo: &PostgresUserAnalysisRepository,
    start_ts: &str,
    end_ts: &str,
    year_month: &str,
) -> Result<(), Error> {
    // Convert times to epoch
    let start_dt = NaiveDateTime::parse_from_str(start_ts, "%Y-%m-%d %H:%M:%S")
        .map_err(|e| Error::Parse(e.to_string()))?;
    let end_dt = NaiveDateTime::parse_from_str(end_ts, "%Y-%m-%d %H:%M:%S")
        .map_err(|e| Error::Parse(e.to_string()))?;
    let start_epoch = to_epoch(start_dt);
    let end_epoch = to_epoch(end_dt);

    // 1) find distinct user_ids in that range
    let user_rows = sqlx::query(
        r#"
        SELECT DISTINCT user_id
        FROM chat_messages
        WHERE timestamp >= $1
          AND timestamp < $2
        "#
    )
        .bind(start_epoch)
        .bind(end_epoch)
        .fetch_all(db.pool())
        .await?;

    // 2) for each user, gather messages, run AI logic
    for row in user_rows {
        let user_id: String = row.try_get("user_id")?;

        let messages = sqlx::query_as::<_, ChatMessage>(
            r#"
            SELECT message_id, platform, channel, user_id,
                   message_text, timestamp, metadata
            FROM chat_messages
            WHERE user_id = $1
              AND timestamp >= $2
              AND timestamp < $3
            "#
        )
            .bind(&user_id)
            .bind(start_epoch)
            .bind(end_epoch)
            .fetch_all(db.pool())
            .await?;

        // run your AI or scoring
        let (spam, intel, quality, horni, summary) = run_ai_scoring(&messages).await;

        // store monthly record in user_analysis_history
        let hist_id = Uuid::new_v4().to_string();
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
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, EXTRACT(EPOCH FROM NOW()))
            "#
        )
            .bind(&hist_id)
            .bind(&user_id)
            .bind(year_month)
            .bind(spam)
            .bind(intel)
            .bind(quality)
            .bind(horni)
            .bind(&summary)
            .execute(db.pool())
            .await?;

        // also update user_analysis with combined or new scores
        if let Some(mut analysis) = user_analysis_repo.get_analysis(&user_id).await? {
            // example weighted average
            analysis.spam_score = 0.7 * analysis.spam_score + 0.3 * spam;
            analysis.intelligibility_score = 0.7 * analysis.intelligibility_score + 0.3 * intel;
            analysis.quality_score = 0.7 * analysis.quality_score + 0.3 * quality;
            analysis.horni_score = 0.7 * analysis.horni_score + 0.3 * horni;

            let old_notes = analysis.ai_notes.clone().unwrap_or_default();
            let appended_notes = format!(
                "{}\n\n=== {} summary ===\n{}",
                old_notes, year_month, summary
            );
            analysis.ai_notes = Some(appended_notes);

            user_analysis_repo.update_analysis(&analysis).await?;
        } else {
            // or create a brand new row if none
            let new_one = UserAnalysis {
                user_analysis_id: Uuid::new_v4().to_string(),
                user_id: user_id.clone(),
                spam_score: spam,
                intelligibility_score: intel,
                quality_score: quality,
                horni_score: horni,
                ai_notes: Some(summary),
                moderator_notes: None,
                created_at: Utc::now().naive_utc(),
                updated_at: Utc::now().naive_utc(),
            };
            user_analysis_repo.create_analysis(&new_one).await?;
        }
    }

    Ok(())
}

/// Fake "AI scoring" routine
async fn run_ai_scoring(
    messages: &[ChatMessage]
) -> (f32, f32, f32, f32, String) {
    let count = messages.len() as f32;
    let spam = 0.1 * count.min(5.0);
    let intel = 0.5;
    let quality = 0.6;
    let horni = 0.2;
    let summary = format!("User posted {} messages. Spam est: {:.2}", count, spam);
    (spam, intel, quality, horni, summary)
}

/// Helper for "YYYY-MM" => returns ("YYYY-MM-01 00:00:00", nextMonthStart).
fn build_month_range(year_month: &str) -> Result<(String, String), Error> {
    let (y, m) = parse_year_month(year_month)?;
    let start_date = NaiveDate::from_ymd_opt(y, m, 1)
        .ok_or_else(|| Error::Parse("Invalid date".into()))?;
    let (ny, nm) = next_month(y, m);
    let end_date = NaiveDate::from_ymd_opt(ny, nm, 1)
        .ok_or_else(|| Error::Parse("Invalid next date".into()))?;

    let start_ts = format!("{} 00:00:00", start_date);
    let end_ts   = format!("{} 00:00:00", end_date);
    Ok((start_ts, end_ts))
}

/// Parse "YYYY-MM" into (year, month).
pub fn parse_year_month(s: &str) -> Result<(i32, u32), Error> {
    if s.len() != 7 || !s.contains('-') {
        return Err(Error::Parse(format!("Not YYYY-MM: {}", s)));
    }
    let y: i32 = s[0..4].parse().map_err(|_| Error::Parse("Bad year".into()))?;
    let m: u32 = s[5..7].parse().map_err(|_| Error::Parse("Bad month".into()))?;
    Ok((y, m))
}

/// Return the next month. E.g. next_month(2025,12) => (2026,1)
fn next_month(year: i32, month: u32) -> (i32, u32) {
    if month == 12 {
        (year + 1, 1)
    } else {
        (year, month + 1)
    }
}