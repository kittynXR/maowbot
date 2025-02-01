use chrono::{NaiveDateTime, TimeZone, Utc};

/// Convert a `NaiveDateTime` to epoch seconds.
/// Old `NaiveDateTime::timestamp()` is deprecated; you can switch to `.and_utc()`.
pub fn to_epoch(dt: NaiveDateTime) -> i64 {
    dt.and_utc().timestamp()
}

/// Convert epoch seconds (i64) to `NaiveDateTime`.
/// Using `NaiveDateTime::from_timestamp_opt(...)`
pub fn from_epoch(epoch: i64) -> NaiveDateTime {
    // If `from_timestamp_opt` returns None, fallback to 1970-01-01
    NaiveDateTime::from_timestamp_opt(epoch, 0)
        .unwrap_or_else(|| NaiveDateTime::from_timestamp_opt(0, 0).unwrap())
}

/// Returns the current epoch seconds.
pub fn current_epoch() -> i64 {
    Utc::now().naive_utc().and_utc().timestamp()
}
