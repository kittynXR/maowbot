// maowbot-core/src/tasks/daily_maintenance.rs

use chrono::{Datelike, NaiveDate, Utc};
use sqlx::{Pool, Postgres, Row};
use tracing::{info, error};

use crate::Error;

/// Spawns a background task that runs once a day (or at a chosen interval)
/// and handles partition creation + old-partition drop.
pub fn spawn_daily_partition_maintenance_task(
    db: crate::db::Database,
    cutoff_days: i64, // e.g. 30
) {
    // Spawn an asynchronous repeating task. In production you might want a more robust cron-like approach.
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(24 * 3600));
        loop {
            interval.tick().await;
            if let Err(e) = run_daily_partition_maintenance(&db, cutoff_days).await {
                error!("Daily partition maintenance failed: {:?}", e);
            }
        }
    });
}

/// Called once a day to (a) create partitions for the current & next month
/// and (b) drop partitions older than `cutoff_days`.
pub async fn run_daily_partition_maintenance(
    db: &crate::db::Database,
    cutoff_days: i64,
) -> Result<(), Error> {
    info!("Starting daily partition maintenance for chat_messages ...");
    let pool = db.pool();

    // (1) Ensure partitions exist for the current month and next month
    let now = Utc::now().naive_utc().date();
    let first_of_current = NaiveDate::from_ymd_opt(now.year(), now.month(), 1)
        .ok_or_else(|| Error::Parse("Could not parse date for current month".to_string()))?;
    let first_of_next = if now.month() == 12 {
        NaiveDate::from_ymd_opt(now.year() + 1, 1, 1)
    } else {
        NaiveDate::from_ymd_opt(now.year(), now.month() + 1, 1)
    }
        .ok_or_else(|| Error::Parse("Could not parse date for next month".to_string()))?;

    create_month_partition_if_needed(pool, first_of_current).await?;
    create_month_partition_if_needed(pool, first_of_next).await?;

    // (2) Drop partitions that are older than the cutoff
    drop_old_chat_partitions(pool, cutoff_days).await?;

    info!("Daily partition maintenance complete.");
    Ok(())
}

/// Helper that creates a monthly partition if it does not already exist.
async fn create_month_partition_if_needed(
    pool: &Pool<Postgres>,
    first_day_of_month: NaiveDate,
) -> Result<(), Error> {
    let year = first_day_of_month.year();
    let month = first_day_of_month.month();

    // Define the boundary from the first day to the first day of next month.
    let next_month = if month == 12 {
        NaiveDate::from_ymd_opt(year + 1, 1, 1).unwrap()
    } else {
        NaiveDate::from_ymd_opt(year, month + 1, 1).unwrap()
    };

    let partition_name = format!("chat_messages_{:04}{:02}", year, month);

    let range_start = first_day_of_month.and_hms_opt(0, 0, 0).unwrap().timestamp();
    let range_end = next_month.and_hms_opt(0, 0, 0).unwrap().timestamp();

    // Here we assume that chat_messages is defined as a RANGE partitioned table on the "timestamp" column.
    let create_sql = format!(
        r#"
        CREATE TABLE IF NOT EXISTS {partition_name}
        PARTITION OF chat_messages
        FOR VALUES FROM ({range_start}) TO ({range_end});
        "#,
        partition_name = partition_name,
        range_start = range_start,
        range_end = range_end,
    );

    // Instead of acquiring a connection, we pass the pool (which implements Executor)
    sqlx::query(&create_sql).execute(pool).await?;
    info!("Partition ensured: {}", partition_name);

    Ok(())
}

/// Drops partitions older than `cutoff_days` by checking their range boundaries.
async fn drop_old_chat_partitions(pool: &Pool<Postgres>, cutoff_days: i64) -> Result<(), Error> {
    // 1) Determine the cutoff timestamp
    let now = Utc::now().timestamp();
    let cutoff_ts = now - (cutoff_days * 86400);

    // 2) Query for child partitions of chat_messages
    let child_partitions_sql = r#"
        SELECT (inhrelid::regclass)::text AS partition_name
        FROM pg_inherits
        WHERE inhparent::regclass = 'chat_messages'::regclass;
    "#;


    let rows = sqlx::query(child_partitions_sql).fetch_all(pool).await?;
    for row in rows {
        let partition_name: String = row.get("partition_name");
        // Expecting a name like "chat_messages_202501"

        // Query the boundary expression for this partition
        let boundary_sql = format!(
            r#"
            SELECT pg_get_expr(relpartbound, oid) AS boundary
            FROM pg_class
            WHERE relname = '{partition_name}';
            "#,
            partition_name = partition_name
        );

        let boundary_val: (Option<String>,) =
            sqlx::query_as(&boundary_sql).fetch_one(pool).await.unwrap_or((None,));

        if let Some(expr_text) = boundary_val.0 {
            // Typically the expression looks like: `FOR VALUES FROM (1672531200) TO (1675209600)`
            if let Some(to_val) = parse_upper_bound(&expr_text) {
                if to_val < cutoff_ts {
                    let drop_sql = format!("DROP TABLE IF EXISTS {};", partition_name);
                    let _ = sqlx::query(&drop_sql).execute(pool).await?;
                    info!("Dropped old partition: {}", partition_name);
                }
            }
        }
    }

    Ok(())
}

/// A very basic parser to extract the upper bound (the value after "TO (") from the boundary expression.
fn parse_upper_bound(bound_expr: &str) -> Option<i64> {
    let s = bound_expr.to_lowercase();
    if let Some(idx) = s.find("to (") {
        let part = &s[(idx + 4)..];
        if let Some(end_paren) = part.find(')') {
            let val_str = part[..end_paren].trim();
            return val_str.parse::<i64>().ok();
        }
    }
    None
}