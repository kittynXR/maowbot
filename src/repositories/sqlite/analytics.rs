use async_trait::async_trait;
use sqlx::{Pool, Sqlite, Row};
use uuid::Uuid;
use chrono::{NaiveDateTime, Utc};
use serde_json::Value;
use sqlx::FromRow;
use crate::Error;

/// Data models (you could put these in separate files):
#[derive(Clone, Debug, FromRow)]
pub struct ChatMessage {
    pub message_id: String,
    pub platform: String,
    pub channel: String,
    pub user_id: String,
    pub message_text: String,

    // The DB column is an integer storing microseconds (or seconds) since epoch:
    pub timestamp: i64,

    pub metadata: Option<Value>,
}

#[derive(Clone)]
pub struct ChatSession {
    pub session_id: String,
    pub platform: String,
    pub channel: String,
    pub user_id: String,
    pub joined_at: NaiveDateTime,
    pub left_at: Option<NaiveDateTime>,
    pub session_duration_seconds: Option<i64>,
}

#[derive(Clone)]
pub struct BotEvent {
    pub event_id: String,
    pub event_type: String,
    pub event_timestamp: NaiveDateTime,
    pub data: Option<Value>,
}

/// Converts a NaiveDateTime to “microseconds since Unix epoch” as i64.
fn datetime_to_epoch_micros(dt: NaiveDateTime) -> i64 {
    // dt.timestamp() gives the whole seconds since epoch as i64
    // dt.timestamp_subsec_micros() is the fractional part in microseconds (u32)
    let secs = dt.timestamp();
    let sub_micros = dt.timestamp_subsec_micros() as i64;
    secs.checked_mul(1_000_000)
        .and_then(|s| s.checked_add(sub_micros))
        // If it overflows (very unlikely), just panic or return something
        .unwrap_or_else(|| panic!("Overflow in datetime_to_epoch_micros"))
}

/// Converts “microseconds since Unix epoch” to a NaiveDateTime.
fn epoch_micros_to_datetime(us: i64) -> Result<NaiveDateTime, Error> {
    // separate into seconds + leftover micros
    let secs = us / 1_000_000;
    let micros = (us % 1_000_000) as u32; // remainder is the microseconds part
    // from_timestamp_opt wants (secs, nanos)
    // so convert micros to nanos
    let nanos = micros * 1000;

    match NaiveDateTime::from_timestamp_opt(secs, nanos) {
        Some(ndt) => Ok(ndt),
        None => Err(Error::Parse("Invalid microsecond timestamp".into())),
    }
}

#[async_trait]
pub trait AnalyticsRepo: Send + Sync + 'static {
    async fn insert_chat_message(&self, msg: &ChatMessage) -> Result<(), Error>;
}

/// Main repository struct
#[derive(Clone)]
pub struct SqliteAnalyticsRepository {
    pool: Pool<Sqlite>,
}

impl SqliteAnalyticsRepository {
    pub fn new(pool: Pool<Sqlite>) -> Self {
        Self { pool }
    }

    // ---------------------------
    // Chat Messages
    // ---------------------------
    /// Inserts a chat message, storing `msg.timestamp` as microseconds in the integer `timestamp` column.
    pub async fn insert_chat_message(&self, msg: &ChatMessage) -> Result<(), Error> {
        let metadata_str = match &msg.metadata {
            Some(val) => val.to_string(),
            None => "".to_string(),
        };

        // Convert the NaiveDateTime to microseconds


        sqlx::query(
            r#"
            INSERT INTO chat_messages (
            message_id, platform, channel, user_id,
            message_text, timestamp, metadata
            ) VALUES (?, ?, ?, ?, ?, ?, ?)

            "#
        )
            .bind(&msg.message_id)
            .bind(&msg.platform)
            .bind(&msg.channel)
            .bind(&msg.user_id)
            .bind(&msg.message_text)
            // Insert the i64 microsecond value
            .bind(msg.timestamp)
            .bind(metadata_str)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    /// Retrieves the most recent N messages for a given platform/channel.
    /// Reads back the integer `timestamp` column, converts to `NaiveDateTime`.
    pub async fn get_recent_messages(
        &self,
        platform: &str,
        channel: &str,
        limit: i64,
    ) -> Result<Vec<ChatMessage>, Error> {
        let rows = sqlx::query(
            r#"
            SELECT
                message_id,
                platform,
                channel,
                user_id,
                message_text,
                timestamp,   -- stored as integer in DB
                metadata
            FROM chat_messages
            WHERE platform = ? AND channel = ?
            ORDER BY timestamp DESC
            LIMIT ?
            "#
        )
            .bind(platform)
            .bind(channel)
            .bind(limit)
            .fetch_all(&self.pool)
            .await?;

        let mut messages = Vec::with_capacity(rows.len());

        for row in rows {
            // read i64 from 'timestamp'
            let epoch_micros: i64 = row.try_get("timestamp")?;
            // convert to NaiveDateTime
            // let timestamp = epoch_micros_to_datetime(epoch_micros)?;

            let meta_str: Option<String> = row.try_get("metadata")?;
            let metadata = meta_str
                .as_deref()
                .and_then(|m| serde_json::from_str(m).ok());

            messages.push(ChatMessage {
                message_id: row.try_get("message_id")?,
                platform: row.try_get("platform")?,
                channel: row.try_get("channel")?,
                user_id: row.try_get("user_id")?,
                message_text: row.try_get("message_text")?,
                timestamp: epoch_micros,
                metadata,
            });
        }

        Ok(messages)
    }

    // ---------------------------
    // Chat Sessions
    // ---------------------------
    /// In a similar way, you would store joined_at and left_at as microseconds.
    /// For brevity, let's show just how you'd insert joined_at.
    pub async fn insert_chat_session(&self, session: &ChatSession) -> Result<(), Error> {
        let joined_micros = datetime_to_epoch_micros(session.joined_at);
        let left_micros = session
            .left_at
            .map(datetime_to_epoch_micros)
            .unwrap_or(0); // or store as NULL if you prefer

        sqlx::query(
            r#"
            INSERT INTO chat_sessions (
                session_id, platform, channel, user_id, joined_at, left_at, session_duration_seconds
            ) VALUES (?, ?, ?, ?, ?, ?, ?)
            "#
        )
            .bind(&session.session_id)
            .bind(&session.platform)
            .bind(&session.channel)
            .bind(&session.user_id)
            .bind(joined_micros) // store as integer
            .bind(left_micros)   // store as integer
            .bind(session.session_duration_seconds)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    pub async fn close_chat_session(
        &self,
        session_id: &str,
        left_at: NaiveDateTime,
        duration_seconds: i64
    ) -> Result<(), Error> {
        let left_micros = datetime_to_epoch_micros(left_at);

        sqlx::query(
            r#"
            UPDATE chat_sessions
            SET left_at = ?, session_duration_seconds = ?
            WHERE session_id = ?
            "#
        )
            .bind(left_micros)
            .bind(duration_seconds)
            .bind(session_id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    // ---------------------------
    // Bot Events
    // ---------------------------
    pub async fn insert_bot_event(&self, event: &BotEvent) -> Result<(), Error> {
        let data_str = event.data.as_ref().map(|d| d.to_string()).unwrap_or_default();

        // convert to microseconds
        let evt_micros = datetime_to_epoch_micros(event.event_timestamp);

        sqlx::query(
            r#"
            INSERT INTO bot_events (
                event_id, event_type, event_timestamp, data
            ) VALUES (?, ?, ?, ?)
            "#
        )
            .bind(&event.event_id)
            .bind(&event.event_type)
            .bind(evt_micros)
            .bind(data_str)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    // Example aggregator for daily stats
    pub async fn update_daily_stats(
        &self,
        date_str: &str,
        new_messages: i64,
        new_visits: i64
    ) -> Result<(), Error> {
        sqlx::query(
            r#"
            INSERT INTO daily_stats (date, total_messages, total_chat_visits)
            VALUES (?, ?, ?)
            ON CONFLICT(date) DO UPDATE
            SET total_messages = daily_stats.total_messages + excluded.total_messages,
                total_chat_visits = daily_stats.total_chat_visits + excluded.total_chat_visits
            "#
        )
            .bind(date_str)
            .bind(new_messages)
            .bind(new_visits)
            .execute(&self.pool)
            .await?;

        Ok(())
    }
}

/// Implementation of the trait
#[async_trait]
impl AnalyticsRepo for SqliteAnalyticsRepository {
    async fn insert_chat_message(&self, msg: &ChatMessage) -> Result<(), Error> {
        self.insert_chat_message(msg).await
    }
}
