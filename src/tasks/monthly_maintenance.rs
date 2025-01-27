// File: src/tasks/monthly_maintenance.rs

use std::path::{Path, PathBuf};
use chrono::{Datelike, NaiveDate, Utc};
use sqlx::{Row};
use uuid::Uuid;
use tracing::info;

use crate::{
    Error,
    Database,
    // We can reference your user analysis and chat_messages structures
    repositories::sqlite::analytics::ChatMessage,
    repositories::sqlite::user_analysis::SqliteUserAnalysisRepository,
    models::UserAnalysis,
};
use crate::repositories::sqlite::UserAnalysisRepository;

/// The main function you’ll call from run_server. Checks if we missed months
/// and if so, archives them plus does AI summarizing, etc.
pub async fn maybe_run_monthly_maintenance(
    db: &Database,
    user_analysis_repo: &SqliteUserAnalysisRepository
) -> Result<(), Error> {
    // 1) Determine which month(s) to archive
    // (Same logic as before: read_last_archived_month, etc.)
    // Suppose we find we need to archive "2025-01" => we do:

    let (start_ts, end_ts) = ("2025-01-01 00:00:00", "2025-02-01 00:00:00");
    let archive_file = PathBuf::from("archives").join("2025-01_archive.db");

    // 2) Summarize or do your user analysis. Then do the row copy:
    archive_one_month_no_attach(db, start_ts, end_ts, &archive_file).await?;

    // 3) Update maintenance_state => "archived_until"= "2025-01"
    sqlx::query(r#"
      INSERT INTO maintenance_state (state_key, state_value)
      VALUES ('archived_until','2025-01')
      ON CONFLICT(state_key) DO UPDATE SET
         state_value=excluded.state_value
    "#)
        .execute(db.pool())
        .await?;

    Ok(())
}

/// Reads the "archived_until" state from some tiny table. Returns e.g. Some("2024-12") or None.
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

/// Writes to maintenance_state: "archived_until" => e.g. "2025-01"
async fn update_last_archived_month(db: &Database, year_month: &str) -> Result<(), Error> {
    sqlx::query(
        r#"
        INSERT INTO maintenance_state (state_key, state_value)
        VALUES ('archived_until', ?)
        ON CONFLICT(state_key) DO UPDATE SET
            state_value = excluded.state_value
        "#
    )
        .bind(year_month)
        .execute(db.pool())
        .await?;
    Ok(())
}

/// If the bot was offline for multiple months, we find all missing months between last_archived and target.
pub fn collect_missing_months(
    last_archived: Option<&str>,
    target: &str
) -> Result<Vec<String>, Error> {
    // If we never archived, we do just the target month this time, or you can choose to go further back, if needed.
    if last_archived.is_none() {
        return Ok(vec![target.to_string()]);
    }

    let (la_y, la_m) = parse_year_month(last_archived.unwrap())?;
    let (tg_y, tg_m) = parse_year_month(target)?;

    // We'll loop from la_y, la_m + 1 up to tg_y, tg_m
    let mut results = Vec::new();

    // Start from the month after last_archived
    let (mut cy, mut cm) = next_month(la_y, la_m);

    while (cy < tg_y) || (cy == tg_y && cm <= tg_m) {
        results.push(format!("{:04}-{:02}", cy, cm));
        let (ny, nm) = next_month(cy, cm);
        cy = ny;
        cm = nm;
    }

    Ok(results)
}

/// Archive data for one month into a separate .db file, plus do monthly AI summarization.
use sqlx::{Sqlite, SqliteConnection, Acquire};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
// add Acquire to help with .acquire()

// ...
pub async fn archive_one_month(
    db: &Database,
    start_ts: &str,
    end_ts: &str,
    archive_path: &Path
) -> Result<(), Error> {
    // Acquire single connection from the pool
    let mut conn = db.pool().acquire().await?;

    // 1) Set locking_mode=EXCLUSIVE
    sqlx::query("PRAGMA locking_mode=EXCLUSIVE")
        .execute(&mut *conn)
        .await?;

    // 2) Set busy_timeout
    sqlx::query("PRAGMA busy_timeout=2000")
        .execute(&mut *conn)
        .await?;

    // 3) Begin EXCLUSIVE
    sqlx::query("BEGIN EXCLUSIVE")
        .execute(&mut *conn)
        .await?;

    // 4) Attach
    let attach_sql = format!("ATTACH '{}' AS archdb", archive_path.display());
    sqlx::query(&attach_sql)
        .execute(&mut *conn)
        .await?;

    // 5) Create table in archdb if needed
    sqlx::query(r#"
        CREATE TABLE IF NOT EXISTS archdb.chat_messages (
            message_id TEXT PRIMARY KEY,
            platform TEXT NOT NULL,
            channel TEXT NOT NULL,
            user_id TEXT NOT NULL,
            message_text TEXT NOT NULL,
            timestamp INTEGER NOT NULL,
            metadata TEXT
        )
    "#)
        .execute(&mut *conn)
        .await?;

    // 6) Insert into archdb
    sqlx::query(r#"
        INSERT INTO archdb.chat_messages
        SELECT *
        FROM main.chat_messages
        WHERE timestamp >= strftime('%s', ?)
          AND timestamp < strftime('%s', ?)
    "#)
        .bind(start_ts)
        .bind(end_ts)
        .execute(&mut *conn)
        .await?;

    // 7) Delete from main
    sqlx::query(r#"
        DELETE FROM main.chat_messages
        WHERE timestamp >= strftime('%s', ?)
          AND timestamp < strftime('%s', ?)
    "#)
        .bind(start_ts)
        .bind(end_ts)
        .execute(&mut *conn)
        .await?;

    // 8) Detach
    sqlx::query("DETACH archdb")
        .execute(&mut *conn)
        .await?;

    // 9) Commit
    sqlx::query("COMMIT")
        .execute(&mut *conn)
        .await?;

    Ok(())
}

pub async fn archive_one_month_no_attach(
    db: &Database,
    start_ts: &str,
    end_ts: &str,
    archive_path: &Path
) -> Result<(), Error> {
    // 1) Gather relevant rows from main DB
    let rows = sqlx::query_as::<_, ChatMessage>(r#"
        SELECT
          message_id,
          platform,
          channel,
          user_id,
          message_text,
          timestamp,
          metadata
        FROM chat_messages
        WHERE timestamp >= strftime('%s', ?)
          AND timestamp < strftime('%s', ?)
    "#)
        .bind(start_ts)
        .bind(end_ts)
        .fetch_all(db.pool())
        .await?;

    if rows.is_empty() {
        return Ok(());
    }

    // 2) Open or create the archive DB as a separate database
    let connect_opts = SqliteConnectOptions::new()
        .filename(archive_path)
        .create_if_missing(true);
    let archive_pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(connect_opts)
        .await?;

    // 3) Possibly create the chat_messages table if not exists
    sqlx::query(r#"
      CREATE TABLE IF NOT EXISTS chat_messages (
        message_id TEXT PRIMARY KEY,
        platform TEXT NOT NULL,
        channel TEXT NOT NULL,
        user_id TEXT NOT NULL,
        message_text TEXT NOT NULL,
        timestamp INTEGER NOT NULL,
        metadata TEXT
      )
    "#)
        .execute(&archive_pool)
        .await?;

    // 4) Insert the rows into the archive DB
    // Optionally wrap in a transaction for performance
    {
        let mut tx = archive_pool.begin().await?;
        for msg in &rows {
            sqlx::query(r#"
              INSERT INTO chat_messages
              (message_id, platform, channel, user_id, message_text, timestamp, metadata)
              VALUES (?, ?, ?, ?, ?, ?, ?)
            "#)
                .bind(&msg.message_id)
                .bind(&msg.platform)
                .bind(&msg.channel)
                .bind(&msg.user_id)
                .bind(&msg.message_text)
                .bind(msg.timestamp)
                .bind(&msg.metadata)
                .execute(&mut *tx)  // note &mut *tx
                .await?;
        }
        tx.commit().await?;
    }

    // 5) Delete from main
    sqlx::query(r#"
      DELETE FROM chat_messages
      WHERE timestamp >= strftime('%s', ?)
        AND timestamp < strftime('%s', ?)
    "#)
        .bind(start_ts)
        .bind(end_ts)
        .execute(db.pool())
        .await?;

    // 6) Optionally close the archive DB
    archive_pool.close().await;

    Ok(())
}

/// Summarizes chat messages from [start_ts, end_ts) => store in user_analysis_history + update user_analysis
async fn generate_monthly_user_summaries(
    db: &Database,
    user_analysis_repo: &SqliteUserAnalysisRepository,
    start_ts: &str,
    end_ts: &str,
    year_month: &str,
) -> Result<(), Error> {
    // 1) find distinct user_ids in that range
    let user_rows = sqlx::query(
        r#"
        SELECT DISTINCT user_id
        FROM chat_messages
        WHERE timestamp >= strftime('%s', ?)
          AND timestamp <  strftime('%s', ?)
        "#
    )
        .bind(start_ts)
        .bind(end_ts)
        .fetch_all(db.pool())
        .await?;

    // 2) for each user, gather messages, run AI logic
    for r in user_rows {
        let user_id: String = r.try_get("user_id")?;

        let messages = sqlx::query_as::<_, ChatMessage>(
            r#"
            SELECT message_id, platform, channel, user_id,
                   message_text, timestamp, metadata
            FROM chat_messages
            WHERE user_id = ?
              AND timestamp >= strftime('%s', ?)
              AND timestamp <  strftime('%s', ?)
            "#
        )
            .bind(&user_id)
            .bind(start_ts)
            .bind(end_ts)
            .fetch_all(db.pool())
            .await?;

        // run your AI or scoring
        let (spam, intel, quality, horni, summary) = run_ai_scoring(&messages).await;

        // store monthly record in user_analysis_history
        // you might define that table in a new migration
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
                ai_notes
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
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

            // append summary to existing notes
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
                created_at: chrono::Utc::now().naive_utc(),
                updated_at: chrono::Utc::now().naive_utc(),
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

/// Helper for "2025-02" => the month starts "2025-02-01" and ends "2025-03-01".
fn build_month_range(year_month: &str) -> Result<(String, String), Error> {
    let (y, m) = parse_year_month(year_month)?;
    let start_date = NaiveDate::from_ymd_opt(y, m, 1)
        .ok_or_else(|| Error::Parse("Invalid date".into()))?;
    let (ny, nm) = next_month(y, m);
    let end_date = NaiveDate::from_ymd_opt(ny, nm, 1)
        .ok_or_else(|| Error::Parse("Invalid next date".into()))?;

    // Format as e.g. "2025-02-01 00:00:00"
    let start_ts = format!("{} 00:00:00", start_date);
    let end_ts   = format!("{} 00:00:00", end_date);
    Ok((start_ts, end_ts))
}

/// For a "YYYY-MM" string, parse out the year & month
pub fn parse_year_month(s: &str) -> Result<(i32, u32), Error> {
    if s.len() != 7 || !s.contains('-') {
        return Err(Error::Parse(format!("Not YYYY-MM: {s}")));
    }
    let y: i32 = s[0..4].parse().map_err(|_| Error::Parse("Bad year".into()))?;
    let m: u32 = s[5..7].parse().map_err(|_| Error::Parse("Bad month".into()))?;
    Ok((y, m))
}

/// Return the previous month. e.g. previous_month(2025, 1) => (2024,12)
fn previous_month(year: i32, month: u32) -> (i32, u32) {
    if month == 1 {
        (year - 1, 12)
    } else {
        (year, month - 1)
    }
}

/// Return the next month. e.g. next_month(2025,12) => (2026,1)
fn next_month(year: i32, month: u32) -> (i32, u32) {
    if month == 12 {
        (year + 1, 1)
    } else {
        (year, month + 1)
    }
}
