// File: maowbot-core/src/repositories/postgres/commands.rs

use std::str::FromStr;
use async_trait::async_trait;
use sqlx::{Pool, Postgres, Row};
use uuid::Uuid;
use chrono::Utc;
use maowbot_common::error::Error;
use maowbot_common::models::command::{Command, CommandUsage};
use maowbot_common::traits::repository_traits::{CommandRepository, CommandUsageRepository};
use maowbot_common::models::platform::Platform;

pub struct PostgresCommandRepository {
    pub pool: Pool<Postgres>,
}

impl PostgresCommandRepository {
    pub fn new(pool: Pool<Postgres>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl CommandRepository for PostgresCommandRepository {
    async fn create_command(&self, cmd: &Command) -> Result<(), Error> {
        sqlx::query(
            r#"
            INSERT INTO commands (
                command_id,
                platform,
                command_name,
                min_role,
                is_active,
                created_at,
                updated_at,
                cooldown_seconds,
                cooldown_warnonce,
                respond_with_credential,
                stream_online_only,
                stream_offline_only,
                active_credential_id
            )
            VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13)
            "#,
        )
            .bind(cmd.command_id)
            .bind(&cmd.platform)
            .bind(&cmd.command_name)
            .bind(&cmd.min_role)
            .bind(cmd.is_active)
            .bind(cmd.created_at)
            .bind(cmd.updated_at)
            .bind(cmd.cooldown_seconds)
            .bind(cmd.cooldown_warnonce)
            .bind(cmd.respond_with_credential)
            .bind(cmd.stream_online_only)
            .bind(cmd.stream_offline_only)
            .bind(cmd.active_credential_id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    async fn get_command_by_id(&self, command_id: Uuid) -> Result<Option<Command>, Error> {
        let row_opt = sqlx::query(
            r#"
            SELECT
                command_id,
                platform,
                command_name,
                min_role,
                is_active,
                created_at,
                updated_at,
                cooldown_seconds,
                cooldown_warnonce,
                respond_with_credential,
                stream_online_only,
                stream_offline_only,
                active_credential_id
            FROM commands
            WHERE command_id = $1
            "#,
        )
            .bind(command_id)
            .fetch_optional(&self.pool)
            .await?;

        if let Some(r) = row_opt {
            let cmd = Command {
                command_id: r.try_get("command_id")?,
                platform: r.try_get("platform")?,
                command_name: r.try_get("command_name")?,
                min_role: r.try_get("min_role")?,
                is_active: r.try_get("is_active")?,
                created_at: r.try_get("created_at")?,
                updated_at: r.try_get("updated_at")?,
                cooldown_seconds: r.try_get("cooldown_seconds")?,
                cooldown_warnonce: r.try_get("cooldown_warnonce")?,
                respond_with_credential: r.try_get("respond_with_credential")?,
                stream_online_only: r.try_get("stream_online_only")?,
                stream_offline_only: r.try_get("stream_offline_only")?,
                active_credential_id: r.try_get("active_credential_id")?,
            };
            Ok(Some(cmd))
        } else {
            Ok(None)
        }
    }

    async fn get_command_by_name(&self, platform: &str, command_name: &str) -> Result<Option<Command>, Error> {
        let row_opt = sqlx::query(
            r#"
            SELECT
                command_id,
                platform,
                command_name,
                min_role,
                is_active,
                created_at,
                updated_at,
                cooldown_seconds,
                cooldown_warnonce,
                respond_with_credential,
                stream_online_only,
                stream_offline_only,
                active_credential_id
            FROM commands
            WHERE LOWER(platform) = LOWER($1)
              AND LOWER(command_name) = LOWER($2)
            "#,
        )
            .bind(platform)
            .bind(command_name)
            .fetch_optional(&self.pool)
            .await?;

        if let Some(r) = row_opt {
            let cmd = Command {
                command_id: r.try_get("command_id")?,
                platform: r.try_get("platform")?,
                command_name: r.try_get("command_name")?,
                min_role: r.try_get("min_role")?,
                is_active: r.try_get("is_active")?,
                created_at: r.try_get("created_at")?,
                updated_at: r.try_get("updated_at")?,
                cooldown_seconds: r.try_get("cooldown_seconds")?,
                cooldown_warnonce: r.try_get("cooldown_warnonce")?,
                respond_with_credential: r.try_get("respond_with_credential")?,
                stream_online_only: r.try_get("stream_online_only")?,
                stream_offline_only: r.try_get("stream_offline_only")?,
                active_credential_id: r.try_get("active_credential_id")?,
            };
            Ok(Some(cmd))
        } else {
            Ok(None)
        }
    }

    async fn list_commands(&self, platform: &str) -> Result<Vec<Command>, Error> {
        let rows = sqlx::query(
            r#"
            SELECT
                command_id,
                platform,
                command_name,
                min_role,
                is_active,
                created_at,
                updated_at,
                cooldown_seconds,
                cooldown_warnonce,
                respond_with_credential,
                stream_online_only,
                stream_offline_only,
                active_credential_id
            FROM commands
            WHERE LOWER(platform) = LOWER($1)
            ORDER BY command_name ASC
            "#,
        )
            .bind(platform)
            .fetch_all(&self.pool)
            .await?;

        let mut cmds = Vec::new();
        for r in rows {
            let c = Command {
                command_id: r.try_get("command_id")?,
                platform: r.try_get("platform")?,
                command_name: r.try_get("command_name")?,
                min_role: r.try_get("min_role")?,
                is_active: r.try_get("is_active")?,
                created_at: r.try_get("created_at")?,
                updated_at: r.try_get("updated_at")?,
                cooldown_seconds: r.try_get("cooldown_seconds")?,
                cooldown_warnonce: r.try_get("cooldown_warnonce")?,
                respond_with_credential: r.try_get("respond_with_credential")?,
                stream_online_only: r.try_get("stream_online_only")?,
                stream_offline_only: r.try_get("stream_offline_only")?,
                active_credential_id: r.try_get("active_credential_id")?,
            };
            cmds.push(c);
        }
        Ok(cmds)
    }

    async fn update_command(&self, cmd: &Command) -> Result<(), Error> {
        sqlx::query(
            r#"
            UPDATE commands
            SET
                platform = $1,
                command_name = $2,
                min_role = $3,
                is_active = $4,
                updated_at = $5,
                cooldown_seconds = $6,
                cooldown_warnonce = $7,
                respond_with_credential = $8,
                stream_online_only = $9,
                stream_offline_only = $10,
                active_credential_id = $11
            WHERE command_id = $12
            "#,
        )
            .bind(&cmd.platform)
            .bind(&cmd.command_name)
            .bind(&cmd.min_role)
            .bind(cmd.is_active)
            .bind(cmd.updated_at)
            .bind(cmd.cooldown_seconds)
            .bind(cmd.cooldown_warnonce)
            .bind(cmd.respond_with_credential)
            .bind(cmd.stream_online_only)
            .bind(cmd.stream_offline_only)
            .bind(cmd.active_credential_id)
            .bind(cmd.command_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn delete_command(&self, command_id: Uuid) -> Result<(), Error> {
        sqlx::query("DELETE FROM commands WHERE command_id = $1")
            .bind(command_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

pub struct PostgresCommandUsageRepository {
    pub pool: Pool<Postgres>,
}

#[async_trait]
impl CommandUsageRepository for PostgresCommandUsageRepository {
    async fn insert_usage(&self, usage: &CommandUsage) -> Result<(), Error> {
        sqlx::query(
            r#"
            INSERT INTO command_usage (
                usage_id,
                command_id,
                user_id,
                used_at,
                channel,
                usage_text,
                metadata
            ) VALUES ($1,$2,$3,$4,$5,$6,$7)
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
            SELECT usage_id, command_id, user_id, used_at, channel, usage_text, metadata
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

        let mut out = Vec::new();
        for r in rows {
            out.push(CommandUsage {
                usage_id: r.try_get("usage_id")?,
                command_id: r.try_get("command_id")?,
                user_id: r.try_get("user_id")?,
                used_at: r.try_get("used_at")?,
                channel: r.try_get("channel")?,
                usage_text: r.try_get("usage_text")?,
                metadata: r.try_get("metadata")?,
            });
        }
        Ok(out)
    }

    async fn list_usage_for_user(&self, user_id: Uuid, limit: i64) -> Result<Vec<CommandUsage>, Error> {
        let rows = sqlx::query(
            r#"
            SELECT usage_id, command_id, user_id, used_at, channel, usage_text, metadata
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

        let mut out = Vec::new();
        for r in rows {
            out.push(CommandUsage {
                usage_id: r.try_get("usage_id")?,
                command_id: r.try_get("command_id")?,
                user_id: r.try_get("user_id")?,
                used_at: r.try_get("used_at")?,
                channel: r.try_get("channel")?,
                usage_text: r.try_get("usage_text")?,
                metadata: r.try_get("metadata")?,
            });
        }
        Ok(out)
    }
}
