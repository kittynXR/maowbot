// src/utils/time.rs

use chrono::{NaiveDateTime, Utc};

/// Converts a NaiveDateTime into epoch seconds.
pub fn to_epoch(dt: NaiveDateTime) -> i64 {
    dt.timestamp()
}

/// Converts epoch seconds into a NaiveDateTime.
pub fn from_epoch(epoch: i64) -> NaiveDateTime {
    NaiveDateTime::from_timestamp_opt(epoch, 0)
        .expect("Valid epoch seconds should yield a valid NaiveDateTime")
}

/// Returns the current epoch seconds.
pub fn current_epoch() -> i64 {
    Utc::now().timestamp()
}
