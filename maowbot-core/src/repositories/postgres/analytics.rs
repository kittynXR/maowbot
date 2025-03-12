// File: maowbot-core/src/repositories/postgres/analytics.rs

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::{Pool, Postgres, QueryBuilder};
use uuid::Uuid;
pub(crate) use maowbot_common::traits::repository_traits::AnalyticsRepo;
pub(crate) use maowbot_common::models::analytics::{BotEvent, ChatMessage, ChatSession};
use crate::Error;



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

    // ----------------------------------------------------------------
    // Single insert
    // ----------------------------------------------------------------
    async fn insert_chat_message(&self, msg: &ChatMessage) -> Result<(), Error> {
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
            .bind(&msg.metadata)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    // ----------------------------------------------------------------
    // Bulk insert for many messages at once
    // ----------------------------------------------------------------
    async fn insert_chat_messages(&self, msgs: &[ChatMessage]) -> Result<(), Error> {
        if msgs.is_empty() {
            return Ok(());
        }

        // Construct the INSERT with columns:
        let mut builder = QueryBuilder::new(
            r#"INSERT INTO chat_messages (
            message_id, platform, channel, user_id,
            message_text, timestamp, metadata
        ) "#
        );

        // Now we say `VALUES ` explicitly, then push each row via `push_values`:
        // builder.push("VALUES ");
        builder.push_values(msgs, |mut row, msg| {
            row.push_bind(msg.message_id)
                .push_bind(&msg.platform)
                .push_bind(&msg.channel)
                .push_bind(msg.user_id)
                .push_bind(&msg.message_text)
                .push_bind(msg.timestamp)
                .push_bind(&msg.metadata);
        });

        // Build and execute
        let query = builder.build();
        query.execute(&self.pool).await?;

        Ok(())
    }

    async fn get_recent_messages(
        &self,
        platform: &str,
        channel: &str,
        limit: i64
    ) -> Result<Vec<ChatMessage>, Error> {
        let rows = sqlx::query_as::<_, ChatMessage>(
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
            "#
        )
            .bind(platform)
            .bind(channel)
            .bind(limit)
            .fetch_all(&self.pool)
            .await?;

        Ok(rows)
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
            "#
        )
            .bind(left_at)
            .bind(duration_seconds)
            .bind(session_id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    async fn insert_bot_event(&self, event: &BotEvent) -> Result<(), Error> {
        // We'll store event.data as JSONB if you like, but for now it's TEXT in the schema.
        // So we could do `.bind(&event.data)` if changed to JSONB.
        let data_str = match &event.data {
            Some(v) => v.to_string(),
            None => String::new(),
        };

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

    async fn get_messages_for_user(
        &self,
        user_id: Uuid,
        limit: i64,
        offset: i64,
        maybe_platform: Option<&str>,
        maybe_channel: Option<&str>,
        maybe_search: Option<&str>,
    ) -> Result<Vec<ChatMessage>, Error> {
        // We'll build dynamic conditions. Then we can just do a query_as! to ChatMessage.
        let mut sql = String::from(
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
            WHERE user_id = $1
            "#,
        );

        let mut binds: Vec<(usize, String)> = Vec::new();
        let mut bind_index = 2;

        if let Some(pl) = maybe_platform {
            sql.push_str(&format!(" AND LOWER(platform) = LOWER(${})", bind_index));
            binds.push((bind_index, pl.to_string()));
            bind_index += 1;
        }
        if let Some(ch) = maybe_channel {
            sql.push_str(&format!(" AND channel = ${}", bind_index));
            binds.push((bind_index, ch.to_string()));
            bind_index += 1;
        }
        if let Some(s) = maybe_search {
            sql.push_str(&format!(" AND message_text ILIKE ${}", bind_index));
            binds.push((bind_index, format!("%{}%", s)));
            bind_index += 1;
        }

        // ORDER + limit/offset
        sql.push_str(&format!(" ORDER BY timestamp DESC LIMIT ${} OFFSET ${}", bind_index, bind_index + 1));

        let mut query = sqlx::query_as::<_, ChatMessage>(&sql).bind(user_id);

        for (_i, val) in &binds {
            query = query.bind(val);
        }
        query = query.bind(limit).bind(offset);

        let rows = query.fetch_all(&self.pool).await?;
        Ok(rows)
    }

    async fn reassign_user_messages(
        &self,
        from_user: Uuid,
        to_user: Uuid
    ) -> Result<u64, Error> {
        let res = sqlx::query(
            r#"
            UPDATE chat_messages
            SET user_id = $2
            WHERE user_id = $1
            "#,
        )
            .bind(from_user)
            .bind(to_user)
            .execute(&self.pool)
            .await?;

        Ok(res.rows_affected())
    }
}