use async_trait::async_trait;
use sqlx::{Pool, Postgres, Row};
use uuid::Uuid;
use chrono::{Utc};
use crate::Error;
use crate::models::Command;

/// Repository trait for Commands
#[async_trait]
pub trait CommandRepository: Send + Sync {
    async fn create_command(&self, cmd: &Command) -> Result<(), Error>;
    async fn get_command_by_id(&self, command_id: Uuid) -> Result<Option<Command>, Error>;
    async fn get_command_by_name(&self, platform: &str, command_name: &str) -> Result<Option<Command>, Error>;
    async fn list_commands(&self, platform: &str) -> Result<Vec<Command>, Error>;
    async fn update_command(&self, cmd: &Command) -> Result<(), Error>;
    async fn delete_command(&self, command_id: Uuid) -> Result<(), Error>;
}

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
                command_id, platform, command_name, min_role,
                is_active, created_at, updated_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            "#,
        )
            .bind(cmd.command_id)
            .bind(&cmd.platform)
            .bind(&cmd.command_name)
            .bind(&cmd.min_role)
            .bind(cmd.is_active)
            .bind(cmd.created_at)
            .bind(cmd.updated_at)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    async fn get_command_by_id(&self, command_id: Uuid) -> Result<Option<Command>, Error> {
        let row_opt = sqlx::query(
            r#"
            SELECT command_id, platform, command_name, min_role,
                   is_active, created_at, updated_at
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
            };
            Ok(Some(cmd))
        } else {
            Ok(None)
        }
    }

    async fn get_command_by_name(&self, platform: &str, command_name: &str) -> Result<Option<Command>, Error> {
        let row_opt = sqlx::query(
            r#"
            SELECT command_id, platform, command_name, min_role,
                   is_active, created_at, updated_at
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
            };
            Ok(Some(cmd))
        } else {
            Ok(None)
        }
    }

    async fn list_commands(&self, platform: &str) -> Result<Vec<Command>, Error> {
        let rows = sqlx::query(
            r#"
            SELECT command_id, platform, command_name, min_role,
                   is_active, created_at, updated_at
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
            };
            cmds.push(cmd);
        }
        Ok(cmds)
    }

    async fn update_command(&self, cmd: &Command) -> Result<(), Error> {
        sqlx::query(
            r#"
            UPDATE commands
            SET min_role = $1,
                is_active = $2,
                updated_at = $3
            WHERE command_id = $4
            "#,
        )
            .bind(&cmd.min_role)
            .bind(cmd.is_active)
            .bind(cmd.updated_at)
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
