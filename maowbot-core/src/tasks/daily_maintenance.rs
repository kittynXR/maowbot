// src/tasks/daily_maintenance.rs

use chrono::{Datelike, NaiveDate, NaiveDateTime, Utc};
use sqlx::{Row, PgPool};
use tracing::{info, error};

use crate::Error;

/// Called daily (e.g. on startup) to:
/// 1) Create partitions for the current month + next month
/// 2) Drop partitions older than 30 days
pub async fn run_daily_partition_maintenance(pool: &PgPool) -> Result<(), Error> {
    let today = Utc::now().date_naive();

    // current month
    let (curr_name, curr_start, curr_end) = monthly_partition_info(today.year(), today.month())?;
    ensure_partition_exists(pool, &curr_name, curr_start, curr_end).await?;

    // next month
    let (ny, nm) = next_month(today.year(), today.month());
    let (next_name, next_start, next_end) = monthly_partition_info(ny, nm)?;
    ensure_partition_exists(pool, &next_name, next_start, next_end).await?;

    // drop partitions older than 30 days
    let cutoff_days = 30;
    drop_old_partitions(pool, cutoff_days).await?;

    Ok(())
}

async fn ensure_partition_exists(
    pool: &PgPool,
    table_name: &str,
    range_start: i64,
    range_end: i64
) -> Result<(), Error> {
    let row = sqlx::query("SELECT to_regclass($1) AS regclass")
        .bind(table_name)
        .fetch_one(pool)
        .await?;

    let exists: Option<String> = row.try_get("regclass")?;
    if exists.is_some() {
        info!("Partition '{}' already exists.", table_name);
        return Ok(());
    }

    info!(
        "Creating partition '{}' for range [{}, {})",
        table_name, range_start, range_end
    );
    let create_sql = format!(
        "CREATE TABLE {child} PARTITION OF chat_messages
         FOR VALUES FROM ({start}) TO ({end});",
        child = table_name,
        start = range_start,
        end = range_end
    );

    sqlx::query(&create_sql)
        .execute(pool)
        .await?;

    info!("Partition '{}' created successfully.", table_name);
    Ok(())
}

/// If end range of a partition is < now - cutoff_days, we drop that partition.
async fn drop_old_partitions(pool: &PgPool, cutoff_days: i64) -> Result<(), Error> {
    let now_epoch = Utc::now().timestamp();
    let cutoff_epoch = now_epoch - cutoff_days * 86400;

    // find all child partitions in 'public' schema that match chat_messages_YYYY_MM
    let rows = sqlx::query(r#"
        SELECT relname
        FROM pg_class
        WHERE relname LIKE 'chat_messages_%'
          AND relnamespace = (
              SELECT oid FROM pg_namespace WHERE nspname = 'public'
          )
    "#)
        .fetch_all(pool)
        .await?;

    let partitions = rows.into_iter()
        .filter_map(|r| r.try_get("relname").ok())
        .collect::<Vec<String>>();

    if partitions.is_empty() {
        info!("No child partitions found to drop.");
        return Ok(());
    }

    let mut to_drop = Vec::new();

    for part in &partitions {
        if let Some((y,m)) = parse_partition_name(part) {
            let (_name, _start, end) = monthly_partition_info(y, m)?;
            if end < cutoff_epoch {
                to_drop.push(part.clone());
            }
        }
    }

    if to_drop.is_empty() {
        info!("No partitions older than cutoff_days={}", cutoff_days);
        return Ok(());
    }

    for part in to_drop {
        let sql = format!("DROP TABLE IF EXISTS {} CASCADE;", part);
        info!("Dropping old partition '{}'", part);
        if let Err(e) = sqlx::query(&sql).execute(pool).await {
            error!("Failed to drop partition '{}': {:?}", part, e);
        } else {
            info!("Partition '{}' dropped.", part);
        }
    }
    Ok(())
}

/// Return (table_name, start_epoch, end_epoch) for year/month partition
fn monthly_partition_info(year: i32, month: u32) -> Result<(String, i64, i64), Error> {
    let table_name = format!("chat_messages_{:04}_{:02}", year, month);
    let start_date = NaiveDate::from_ymd_opt(year, month, 1)
        .ok_or_else(|| Error::Parse("Invalid partition date".into()))?
        .and_hms_opt(0,0,0)
        .ok_or_else(|| Error::Parse("Invalid partition date hour/min/sec".into()))?;
    let (ny, nm) = next_month(year, month);
    let end_date = NaiveDate::from_ymd_opt(ny, nm, 1)
        .ok_or_else(|| Error::Parse("Invalid next partition date".into()))?
        .and_hms_opt(0,0,0)
        .ok_or_else(|| Error::Parse("Invalid next partition date hour/min/sec".into()))?;

    let start_epoch = start_date.and_utc().timestamp();
    let end_epoch   = end_date.and_utc().timestamp();

    Ok((table_name, start_epoch, end_epoch))
}

/// Return next month for e.g. (2025,12) => (2026,1)
fn next_month(year: i32, month: u32) -> (i32, u32) {
    if month == 12 {
        (year + 1, 1)
    } else {
        (year, month + 1)
    }
}

/// parse "chat_messages_YYYY_MM"
fn parse_partition_name(name: &str) -> Option<(i32, u32)> {
    let parts: Vec<&str> = name.split('_').collect();
    if parts.len() == 3 {
        let yr = parts[1].parse::<i32>().ok()?;
        let mo = parts[2].parse::<u32>().ok()?;
        Some((yr, mo))
    } else {
        None
    }
}