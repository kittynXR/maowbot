// src/repositories/sqlite/analytics.rs

use async_trait::async_trait;
use sqlx::{Pool, Sqlite, Row};
use uuid::Uuid;
use chrono::{NaiveDateTime, Utc};
use serde_json::Value;

use crate::Error;

/// Data models (you could put these in separate files):
#[derive(Clone)]
pub struct ChatMessage {
    pub message_id: String,
    pub platform: String,
    pub channel: String,
    pub user_id: String,
    pub message_text: String,
    pub timestamp: NaiveDateTime,
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
    pub async fn insert_chat_message(&self, msg: &ChatMessage) -> Result<(), Error> {
        let metadata_str = match &msg.metadata {
            Some(val) => val.to_string(),
            None => "".to_string(),
        };

        sqlx::query(
            r#"
            INSERT INTO chat_messages (
                message_id, platform, channel, user_id, message_text, timestamp, metadata
            ) VALUES (?, ?, ?, ?, ?, ?, ?)
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
                timestamp,
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

        let mut messages = Vec::new();

        for row in rows {
            let message_id: String = row.try_get("message_id")?;
            let platform: String = row.try_get("platform")?;
            let channel: String = row.try_get("channel")?;
            let user_id: String = row.try_get("user_id")?;
            let message_text: String = row.try_get("message_text")?;
            let timestamp: NaiveDateTime = row.try_get("timestamp")?;
            let meta_str: Option<String> = row.try_get("metadata")?;
            let metadata = meta_str
                .as_deref()
                .and_then(|m| serde_json::from_str(m).ok());

            messages.push(ChatMessage {
                message_id,
                platform,
                channel,
                user_id,
                message_text,
                timestamp,
                metadata,
            });
        }

        Ok(messages)
    }

    // ---------------------------
    // Chat Sessions
    // ---------------------------
    pub async fn insert_chat_session(&self, session: &ChatSession) -> Result<(), Error> {
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
            .bind(session.joined_at)
            .bind(session.left_at)
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
        sqlx::query(
            r#"
            UPDATE chat_sessions
            SET left_at = ?, session_duration_seconds = ?
            WHERE session_id = ?
            "#
        )
            .bind(left_at)
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
        sqlx::query(
            r#"
            INSERT INTO bot_events (
                event_id, event_type, event_timestamp, data
            ) VALUES (?, ?, ?, ?)
            "#
        )
            .bind(&event.event_id)
            .bind(&event.event_type)
            .bind(event.event_timestamp)
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

#[async_trait]
impl AnalyticsRepo for SqliteAnalyticsRepository {
    async fn insert_chat_message(&self, msg: &ChatMessage) -> Result<(), Error> {
        self.insert_chat_message(msg).await
    }
}
