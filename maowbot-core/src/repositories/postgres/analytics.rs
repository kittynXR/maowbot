// src/repositories/postgres/analytics.rs
use async_trait::async_trait;
use sqlx::{Pool, Postgres, Row};
use uuid::Uuid;
use chrono::{NaiveDateTime, Utc};
use serde_json::Value;
use sqlx::FromRow;
use crate::utils::time::{to_epoch, from_epoch};
use crate::Error;

/// Represents a single chat message row.
#[derive(Clone, Debug, FromRow)]
pub struct ChatMessage {
    pub message_id: String,
    pub platform: String,
    pub channel: String,
    pub user_id: String,
    pub message_text: String,
    pub timestamp: i64,
    pub metadata: Option<Value>,
}

/// Represents a chat session (user's join/leave times).
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

/// Represents an arbitrary bot event (for logging or analytics).
#[derive(Clone)]
pub struct BotEvent {
    pub event_id: String,
    pub event_type: String,
    pub event_timestamp: NaiveDateTime,
    pub data: Option<Value>,
}

/// Defines the analytics repository trait.
#[async_trait]
pub trait AnalyticsRepo: Send + Sync + 'static {
    /// Insert a new `ChatMessage`.
    async fn insert_chat_message(&self, msg: &ChatMessage) -> Result<(), Error>;

    /// Return up to `limit` most recent messages for a given platform/channel.
    async fn get_recent_messages(
        &self,
        platform: &str,
        channel: &str,
        limit: i64
    ) -> Result<Vec<ChatMessage>, Error>;

    /// Create a new `ChatSession`.
    async fn insert_chat_session(&self, session: &ChatSession) -> Result<(), Error>;

    /// Closes a chat session with `left_at` time and final duration.
    async fn close_chat_session(
        &self,
        session_id: &str,
        left_at: NaiveDateTime,
        duration_seconds: i64
    ) -> Result<(), Error>;

    /// Insert a new `BotEvent`.
    async fn insert_bot_event(&self, event: &BotEvent) -> Result<(), Error>;

    /// Update daily stats row, incrementing messages/visits for a given date.
    async fn update_daily_stats(
        &self,
        date_str: &str,
        new_messages: i64,
        new_visits: i64
    ) -> Result<(), Error>;
}

/// Postgres-based analytics repository.
#[derive(Clone)]
pub struct PostgresAnalyticsRepository {
    pool: Pool<Postgres>,
}

impl PostgresAnalyticsRepository {
    pub fn new(pool: Pool<Postgres>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl AnalyticsRepo for PostgresAnalyticsRepository {
    async fn insert_chat_message(&self, msg: &ChatMessage) -> Result<(), Error> {
        let metadata_str = match &msg.metadata {
            Some(val) => val.to_string(),
            None => "".to_string(),
        };

        sqlx::query(
            r#"
            INSERT INTO chat_messages (
                message_id, platform, channel, user_id,
                message_text, timestamp, metadata
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            "#
        )
            .bind(&msg.message_id)
            .bind(&msg.platform)
            .bind(&msg.channel)
            .bind(&msg.user_id)
            .bind(&msg.message_text)
            .bind(msg.timestamp)
            .bind(metadata_str)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    async fn get_recent_messages(
        &self,
        platform: &str,
        channel: &str,
        limit: i64
    ) -> Result<Vec<ChatMessage>, Error> {
        let rows = sqlx::query(
            r#"
            SELECT
                message_id,
                platform,
                channel,
                user_id,
                message_text,
                timestamp,
                metadata
            FROM chat_messages
            WHERE platform = $1 AND channel = $2
            ORDER BY timestamp DESC
            LIMIT $3
            "#,
        )
            .bind(platform)
            .bind(channel)
            .bind(limit)
            .fetch_all(&self.pool)
            .await?;

        let mut messages = Vec::with_capacity(rows.len());
        for row in rows {
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
                timestamp: row.try_get("timestamp")?,
                metadata,
            });
        }
        Ok(messages)
    }

    async fn insert_chat_session(&self, session: &ChatSession) -> Result<(), Error> {
        let joined_epoch = to_epoch(session.joined_at);
        let left_epoch = session.left_at.map(to_epoch).unwrap_or(0);

        sqlx::query(
            r#"
            INSERT INTO chat_sessions (
                session_id, platform, channel, user_id,
                joined_at, left_at, session_duration_seconds
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            "#
        )
            .bind(&session.session_id)
            .bind(&session.platform)
            .bind(&session.channel)
            .bind(&session.user_id)
            .bind(joined_epoch)
            .bind(left_epoch)
            .bind(session.session_duration_seconds)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    async fn close_chat_session(
        &self,
        session_id: &str,
        left_at: NaiveDateTime,
        duration_seconds: i64
    ) -> Result<(), Error> {
        let left_epoch = to_epoch(left_at);

        sqlx::query(
            r#"
            UPDATE chat_sessions
            SET left_at = $1,
                session_duration_seconds = $2
            WHERE session_id = $3
            "#
        )
            .bind(left_epoch)
            .bind(duration_seconds)
            .bind(session_id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    async fn insert_bot_event(&self, event: &BotEvent) -> Result<(), Error> {
        let data_str = event.data.as_ref().map(|d| d.to_string()).unwrap_or_default();
        let evt_epoch = to_epoch(event.event_timestamp);

        sqlx::query(
            r#"
            INSERT INTO bot_events (
                event_id, event_type, event_timestamp, data
            )
            VALUES ($1, $2, $3, $4)
            "#
        )
            .bind(&event.event_id)
            .bind(&event.event_type)
            .bind(evt_epoch)
            .bind(data_str)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    async fn update_daily_stats(
        &self,
        date_str: &str,
        new_messages: i64,
        new_visits: i64
    ) -> Result<(), Error> {
        // Postgres "ON CONFLICT" upsert
        sqlx::query(
            r#"
            INSERT INTO daily_stats (date, total_messages, total_chat_visits)
            VALUES ($1, $2, $3)
            ON CONFLICT (date) DO UPDATE
              SET total_messages = daily_stats.total_messages + EXCLUDED.total_messages,
                  total_chat_visits = daily_stats.total_chat_visits + EXCLUDED.total_chat_visits
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