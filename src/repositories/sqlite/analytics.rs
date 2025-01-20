// src/repositories/sqlite/analytics.rs

use async_trait::async_trait;
use sqlx::{Pool, Sqlite};
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

        sqlx::query!(
            r#"
            INSERT INTO chat_messages (
                message_id, platform, channel, user_id, message_text, timestamp, metadata
            ) VALUES (?, ?, ?, ?, ?, ?, ?)
            "#,
            msg.message_id,
            msg.platform,
            msg.channel,
            msg.user_id,
            msg.message_text,
            msg.timestamp,
            metadata_str
        )
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
        // Using `sqlx::query!` still, but ensure the local DB is migrated
        let rows = sqlx::query!(
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
        "#,
        platform,
        channel,
        limit
    )
            .fetch_all(&self.pool)
            .await?;

        let mut messages = Vec::new();

        for row in rows {
            // row.metadata is Option<String>
            let metadata_val = row
                .metadata
                .as_deref()                        // Option<&str>
                .and_then(|m| serde_json::from_str(m).ok());

            messages.push(ChatMessage {
                message_id: row.message_id,
                platform: row.platform,
                channel: row.channel,
                user_id: row.user_id,
                message_text: row.message_text,
                timestamp: row.timestamp,
                metadata: metadata_val,
            });
        }

        Ok(messages)
    }


    // ---------------------------
    // Chat Sessions
    // ---------------------------
    /// Create a session row when user joins.
    pub async fn insert_chat_session(&self, session: &ChatSession) -> Result<(), Error> {
        sqlx::query!(
            r#"
            INSERT INTO chat_sessions (
                session_id, platform, channel, user_id, joined_at, left_at, session_duration_seconds
            ) VALUES (?, ?, ?, ?, ?, ?, ?)
            "#,
            session.session_id,
            session.platform,
            session.channel,
            session.user_id,
            session.joined_at,
            session.left_at,
            session.session_duration_seconds
        )
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    /// Update session when user leaves (set left_at and session_duration).
    pub async fn close_chat_session(
        &self,
        session_id: &str,
        left_at: NaiveDateTime,
        duration_seconds: i64
    ) -> Result<(), Error> {
        sqlx::query!(
            r#"
            UPDATE chat_sessions
            SET left_at = ?, session_duration_seconds = ?
            WHERE session_id = ?
            "#,
            left_at,
            duration_seconds,
            session_id
        )
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    // ---------------------------
    // Bot Events
    // ---------------------------
    pub async fn insert_bot_event(&self, event: &BotEvent) -> Result<(), Error> {
        let data_str = event.data.as_ref().map(|d| d.to_string()).unwrap_or_default();
        sqlx::query!(
            r#"
            INSERT INTO bot_events (
                event_id, event_type, event_timestamp, data
            ) VALUES (?, ?, ?, ?)
            "#,
            event.event_id,
            event.event_type,
            event.event_timestamp,
            data_str
        )
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // Example aggregator for daily stats
    /// Summarizes how many messages and chat visits happened so far today.
    /// This is just an example of how you might do an "upsert" on `daily_stats`.
    pub async fn update_daily_stats(
        &self,
        date_str: &str,
        new_messages: i64,
        new_visits: i64
    ) -> Result<(), Error> {
        sqlx::query!(
            r#"
            INSERT INTO daily_stats (date, total_messages, total_chat_visits)
            VALUES (?, ?, ?)
            ON CONFLICT(date) DO UPDATE
            SET total_messages = daily_stats.total_messages + excluded.total_messages,
                total_chat_visits = daily_stats.total_chat_visits + excluded.total_chat_visits
            "#,
            date_str,
            new_messages,
            new_visits
        )
            .execute(&self.pool)
            .await?;

        Ok(())
    }
}
