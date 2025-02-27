use async_trait::async_trait;
use sqlx::{Pool, Postgres, Row};
use uuid::Uuid;
use chrono::Utc;
use crate::Error;
use crate::models::CommandUsage;

/// Repository for storing command usage logs
#[async_trait]
pub trait CommandUsageRepository: Send + Sync {
    async fn insert_usage(&self, usage: &CommandUsage) -> Result<(), Error>;
    async fn list_usage_for_command(&self, command_id: Uuid, limit: i64) -> Result<Vec<CommandUsage>, Error>;
    async fn list_usage_for_user(&self, user_id: Uuid, limit: i64) -> Result<Vec<CommandUsage>, Error>;
}

#[derive(Clone)]
pub struct PostgresCommandUsageRepository {
    pool: Pool<Postgres>,
}

impl PostgresCommandUsageRepository {
    pub fn new(pool: Pool<Postgres>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl CommandUsageRepository for PostgresCommandUsageRepository {
    async fn insert_usage(&self, usage: &CommandUsage) -> Result<(), Error> {
        sqlx::query(
            r#"
            INSERT INTO command_usage (
                usage_id, command_id, user_id, used_at,
                channel, usage_text, metadata
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            "#,
        )
            .bind(usage.usage_id)
            .bind(usage.command_id)
            .bind(usage.user_id)
            .bind(usage.used_at)
            .bind(&usage.channel)
            .bind(&usage.usage_text)
            .bind(&usage.metadata)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    async fn list_usage_for_command(&self, command_id: Uuid, limit: i64) -> Result<Vec<CommandUsage>, Error> {
        let rows = sqlx::query(
            r#"
            SELECT usage_id, command_id, user_id, used_at,
                   channel, usage_text, metadata
            FROM command_usage
            WHERE command_id = $1
            ORDER BY used_at DESC
            LIMIT $2
            "#,
        )
            .bind(command_id)
            .bind(limit)
            .fetch_all(&self.pool)
            .await?;

        let mut results = Vec::new();
        for row in rows {
            let cu = CommandUsage {
                usage_id: row.try_get("usage_id")?,
                command_id: row.try_get("command_id")?,
                user_id: row.try_get("user_id")?,
                used_at: row.try_get("used_at")?,
                channel: row.try_get("channel")?,
                usage_text: row.try_get("usage_text")?,
                metadata: row.try_get("metadata")?,
            };
            results.push(cu);
        }
        Ok(results)
    }

    async fn list_usage_for_user(&self, user_id: Uuid, limit: i64) -> Result<Vec<CommandUsage>, Error> {
        let rows = sqlx::query(
            r#"
            SELECT usage_id, command_id, user_id, used_at,
                   channel, usage_text, metadata
            FROM command_usage
            WHERE user_id = $1
            ORDER BY used_at DESC
            LIMIT $2
            "#,
        )
            .bind(user_id)
            .bind(limit)
            .fetch_all(&self.pool)
            .await?;

        let mut results = Vec::new();
        for row in rows {
            let cu = CommandUsage {
                usage_id: row.try_get("usage_id")?,
                command_id: row.try_get("command_id")?,
                user_id: row.try_get("user_id")?,
                used_at: row.try_get("used_at")?,
                channel: row.try_get("channel")?,
                usage_text: row.try_get("usage_text")?,
                metadata: row.try_get("metadata")?,
            };
            results.push(cu);
        }
        Ok(results)
    }
}
