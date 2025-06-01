use sqlx::{Pool, Postgres, Row};
use async_trait::async_trait;
use uuid::Uuid;
use chrono::{Utc};
use maowbot_common::models::platform::PlatformConfig;
pub(crate) use maowbot_common::traits::repository_traits::PlatformConfigRepository;
use crate::Error;

#[derive(Clone)]
pub struct PostgresPlatformConfigRepository {
    pub pool: Pool<Postgres>,
}

impl PostgresPlatformConfigRepository {
    pub fn new(pool: Pool<Postgres>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl PlatformConfigRepository for PostgresPlatformConfigRepository {
    async fn upsert_platform_config(
        &self,
        platform: &str,
        client_id: Option<String>,
        client_secret: Option<String>,
    ) -> Result<(), Error> {
        let now = Utc::now();

        let existing = self.get_by_platform(platform).await?;
        if let Some(pc) = existing {
            // update
            sqlx::query(
                r#"
                UPDATE platform_config
                SET client_id = $1,
                    client_secret = $2,
                    updated_at = $3
                WHERE platform_config_id = $4
                "#
            )
                .bind(client_id)
                .bind(client_secret)
                .bind(now)
                .bind(pc.platform_config_id)
                .execute(&self.pool)
                .await?;
        } else {
            // insert
            let new_id = Uuid::new_v4();
            sqlx::query(
                r#"
                INSERT INTO platform_config (
                    platform_config_id,
                    platform,
                    client_id,
                    client_secret,
                    created_at,
                    updated_at
                )
                VALUES ($1, $2, $3, $4, $5, $6)
                "#
            )
                .bind(new_id)
                .bind(platform)
                .bind(client_id)
                .bind(client_secret)
                .bind(now)
                .bind(now)
                .execute(&self.pool)
                .await?;
        }
        Ok(())
    }

    async fn get_platform_config(&self, platform_config_id: Uuid) -> Result<Option<PlatformConfig>, Error> {
        let row = sqlx::query(
            r#"
            SELECT
                platform_config_id,
                platform,
                client_id,
                client_secret,
                created_at,
                updated_at
            FROM platform_config
            WHERE platform_config_id = $1
            "#,
        )
            .bind(platform_config_id)
            .fetch_optional(&self.pool)
            .await?;

        if let Some(r) = row {
            let pc = PlatformConfig {
                platform_config_id: r.try_get("platform_config_id")?,
                platform: r.try_get("platform")?,
                client_id: r.try_get("client_id")?,
                client_secret: r.try_get("client_secret")?,
                created_at: r.try_get("created_at")?,
                updated_at: r.try_get("updated_at")?,
            };
            Ok(Some(pc))
        } else {
            Ok(None)
        }
    }

    async fn list_platform_configs(&self, maybe_platform: Option<&str>) -> Result<Vec<PlatformConfig>, Error> {
        // If a platform filter is provided, do case-insensitive match
        let rows = if let Some(p) = maybe_platform {
            sqlx::query(
                r#"
                SELECT
                    platform_config_id,
                    platform,
                    client_id,
                    client_secret,
                    created_at,
                    updated_at
                FROM platform_config
                WHERE LOWER(platform) = LOWER($1)
                ORDER BY created_at DESC
                "#
            )
                .bind(p)
                .fetch_all(&self.pool)
                .await?
        } else {
            sqlx::query(
                r#"
                SELECT
                    platform_config_id,
                    platform,
                    client_id,
                    client_secret,
                    created_at,
                    updated_at
                FROM platform_config
                ORDER BY created_at DESC
                "#
            )
                .fetch_all(&self.pool)
                .await?
        };

        let mut result = Vec::new();
        for r in rows {
            result.push(PlatformConfig {
                platform_config_id: r.try_get("platform_config_id")?,
                platform: r.try_get("platform")?,
                client_id: r.try_get("client_id")?,
                client_secret: r.try_get("client_secret")?,
                created_at: r.try_get("created_at")?,
                updated_at: r.try_get("updated_at")?,
            });
        }
        Ok(result)
    }

    async fn delete_platform_config(&self, platform_config_id: Uuid) -> Result<(), Error> {
        sqlx::query(
            r#"
            DELETE FROM platform_config
            WHERE platform_config_id = $1
            "#,
        )
            .bind(platform_config_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn get_by_platform(&self, platform: &str) -> Result<Option<PlatformConfig>, Error> {
        // case-insensitive
        let row = sqlx::query(
            r#"
            SELECT
                platform_config_id,
                platform,
                client_id,
                client_secret,
                created_at,
                updated_at
            FROM platform_config
            WHERE LOWER(platform) = LOWER($1)
            LIMIT 1
            "#
        )
            .bind(platform)
            .fetch_optional(&self.pool)
            .await?;

        if let Some(r) = row {
            let pc = PlatformConfig {
                platform_config_id: r.try_get("platform_config_id")?,
                platform: r.try_get("platform")?,
                client_id: r.try_get("client_id")?,
                client_secret: r.try_get("client_secret")?,
                created_at: r.try_get("created_at")?,
                updated_at: r.try_get("updated_at")?,
            };
            Ok(Some(pc))
        } else {
            Ok(None)
        }
    }

    async fn count_for_platform(&self, platform: &str) -> Result<i64, Error> {
        let row = sqlx::query(
            r#"
            SELECT COUNT(*) AS count
            FROM platform_config
            WHERE LOWER(platform) = LOWER($1)
            "#,
        )
            .bind(platform)
            .fetch_one(&self.pool)
            .await?;
        let c: i64 = row.try_get("count")?;
        Ok(c)
    }
}