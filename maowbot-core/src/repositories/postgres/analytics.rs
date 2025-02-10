// src/repositories/postgres/analytics.rs

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::{Pool, Postgres, Row, FromRow};
use serde_json::Value;
use uuid::Uuid;
use crate::Error;

#[derive(Clone, Debug)]
#[derive(sqlx::FromRow)]
pub struct ChatMessage {
    pub message_id: Uuid,
    pub platform: String,
    pub channel: String,
    pub user_id: Uuid,
    pub message_text: String,
    pub timestamp: DateTime<Utc>,
    pub metadata: Option<Value>,
}

#[derive(Clone, Debug)]
pub struct ChatSession {
    pub session_id: Uuid,
    pub platform: String,
    pub channel: String,
    pub user_id: Uuid,
    pub joined_at: DateTime<Utc>,
    pub left_at: Option<DateTime<Utc>>,
    pub session_duration_seconds: Option<i64>,
}

/// Arbitrary bot event
#[derive(Clone, Debug)]
pub struct BotEvent {
    pub event_id: Uuid,
    pub event_type: String,
    pub event_timestamp: DateTime<Utc>,
    pub data: Option<Value>,
}

#[async_trait]
pub trait AnalyticsRepo: Send + Sync {
    async fn insert_chat_message(&self, msg: &ChatMessage) -> Result<(), Error>;
    async fn get_recent_messages(
        &self,
        platform: &str,
        channel: &str,
        limit: i64
    ) -> Result<Vec<ChatMessage>, Error>;

    async fn insert_chat_session(&self, session: &ChatSession) -> Result<(), Error>;
    async fn close_chat_session(
        &self,
        session_id: Uuid,
        left_at: DateTime<Utc>,
        duration_seconds: i64
    ) -> Result<(), Error>;

    async fn insert_bot_event(&self, event: &BotEvent) -> Result<(), Error>;

    async fn update_daily_stats(
        &self,
        date_str: &str,
        new_messages: i64,
        new_visits: i64
    ) -> Result<(), Error>;
}

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
            "#,
        )
            .bind(msg.message_id)
            .bind(&msg.platform)
            .bind(&msg.channel)
            .bind(msg.user_id)
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
            WHERE platform = $1
              AND channel = $2
            ORDER BY timestamp DESC
            LIMIT $3
            "#,
        )
            .bind(platform)
            .bind(channel)
            .bind(limit)
            .fetch_all(&self.pool)
            .await?;

        let mut messages = Vec::new();
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
        sqlx::query(
            r#"
            INSERT INTO chat_sessions (
                session_id, platform, channel, user_id,
                joined_at, left_at, session_duration_seconds
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            "#,
        )
            .bind(session.session_id)
            .bind(&session.platform)
            .bind(&session.channel)
            .bind(session.user_id)
            .bind(session.joined_at)
            .bind(session.left_at)
            .bind(session.session_duration_seconds)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    async fn close_chat_session(
        &self,
        session_id: Uuid,
        left_at: DateTime<Utc>,
        duration_seconds: i64
    ) -> Result<(), Error> {
        sqlx::query(
            r#"
            UPDATE chat_sessions
            SET left_at = $1,
                session_duration_seconds = $2
            WHERE session_id = $3
            "#,
        )
            .bind(left_at)
            .bind(duration_seconds)
            .bind(session_id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    async fn insert_bot_event(&self, event: &BotEvent) -> Result<(), Error> {
        let data_str = event.data.as_ref().map(|d| d.to_string()).unwrap_or_default();

        sqlx::query(
            r#"
            INSERT INTO bot_events (
                event_id, event_type, event_timestamp, data
            )
            VALUES ($1, $2, $3, $4)
            "#,
        )
            .bind(event.event_id)
            .bind(&event.event_type)
            .bind(event.event_timestamp)
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
        sqlx::query(
            r#"
            INSERT INTO daily_stats (date, total_messages, total_chat_visits)
            VALUES ($1, $2, $3)
            ON CONFLICT (date) DO UPDATE
              SET total_messages = daily_stats.total_messages + EXCLUDED.total_messages,
                  total_chat_visits = daily_stats.total_chat_visits + EXCLUDED.total_chat_visits
            "#,
        )
            .bind(date_str)
            .bind(new_messages)
            .bind(new_visits)
            .execute(&self.pool)
            .await?;

        Ok(())
    }
}