use async_trait::async_trait;
use sqlx::{Pool, Postgres, Row};
use uuid::Uuid;
use maowbot_common::models::Command;
pub(crate) use maowbot_common::traits::repository_traits::CommandRepository;
use crate::Error;


/// Repository trait for Commands


#[derive(Clone)]
pub struct PostgresCommandRepository {
    pool: Pool<Postgres>,
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
                stream_offline_only
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7,
                    $8, $9, $10, $11, $12)
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
                stream_offline_only

            FROM commands
            WHERE command_id = $1
            "#,
        )
            .bind(command_id)
            .fetch_optional(&self.pool)
            .await?;

        if let Some(row) = row_opt {
            let cmd = Command {
                command_id: row.try_get("command_id")?,
                platform: row.try_get("platform")?,
                command_name: row.try_get("command_name")?,
                min_role: row.try_get("min_role")?,
                is_active: row.try_get("is_active")?,
                created_at: row.try_get("created_at")?,
                updated_at: row.try_get("updated_at")?,

                cooldown_seconds: row.try_get("cooldown_seconds")?,
                cooldown_warnonce: row.try_get("cooldown_warnonce")?,
                respond_with_credential: row.try_get("respond_with_credential")?,
                stream_online_only: row.try_get("stream_online_only")?,
                stream_offline_only: row.try_get("stream_offline_only")?,
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
                stream_offline_only

            FROM commands
            WHERE platform = $1
              AND LOWER(command_name) = LOWER($2)
            "#,
        )
            .bind(platform)
            .bind(command_name)
            .fetch_optional(&self.pool)
            .await?;

        if let Some(row) = row_opt {
            let cmd = Command {
                command_id: row.try_get("command_id")?,
                platform: row.try_get("platform")?,
                command_name: row.try_get("command_name")?,
                min_role: row.try_get("min_role")?,
                is_active: row.try_get("is_active")?,
                created_at: row.try_get("created_at")?,
                updated_at: row.try_get("updated_at")?,

                cooldown_seconds: row.try_get("cooldown_seconds")?,
                cooldown_warnonce: row.try_get("cooldown_warnonce")?,
                respond_with_credential: row.try_get("respond_with_credential")?,
                stream_online_only: row.try_get("stream_online_only")?,
                stream_offline_only: row.try_get("stream_offline_only")?,
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
                stream_offline_only
            FROM commands
            WHERE platform = $1
            ORDER BY command_name ASC
            "#,
        )
            .bind(platform)
            .fetch_all(&self.pool)
            .await?;

        let mut cmds = Vec::new();
        for row in rows {
            let cmd = Command {
                command_id: row.try_get("command_id")?,
                platform: row.try_get("platform")?,
                command_name: row.try_get("command_name")?,
                min_role: row.try_get("min_role")?,
                is_active: row.try_get("is_active")?,
                created_at: row.try_get("created_at")?,
                updated_at: row.try_get("updated_at")?,

                cooldown_seconds: row.try_get("cooldown_seconds")?,
                cooldown_warnonce: row.try_get("cooldown_warnonce")?,
                respond_with_credential: row.try_get("respond_with_credential")?,
                stream_online_only: row.try_get("stream_online_only")?,
                stream_offline_only: row.try_get("stream_offline_only")?,
            };
            cmds.push(cmd);
        }
        Ok(cmds)
    }

    async fn update_command(&self, cmd: &Command) -> Result<(), Error> {
        sqlx::query(
            r#"
            UPDATE commands
            SET
                min_role = $1,
                is_active = $2,
                updated_at = $3,

                cooldown_seconds = $4,
                cooldown_warnonce = $5,
                respond_with_credential = $6,
                stream_online_only = $7,
                stream_offline_only = $8

            WHERE command_id = $9
            "#,
        )
            .bind(&cmd.min_role)
            .bind(cmd.is_active)
            .bind(cmd.updated_at)

            .bind(cmd.cooldown_seconds)
            .bind(cmd.cooldown_warnonce)
            .bind(cmd.respond_with_credential)
            .bind(cmd.stream_online_only)
            .bind(cmd.stream_offline_only)

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