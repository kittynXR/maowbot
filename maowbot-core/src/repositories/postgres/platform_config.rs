// maowbot-core/src/repositories/postgres/platform_config.rs

use sqlx::{Pool, Postgres, Row};
use async_trait::async_trait;
use uuid::Uuid;
use chrono::{DateTime, Utc};
use crate::Error;

/// Basic struct representing a row in `platform_config`, storing
/// client_id/secret for a particular platform. We store exactly
/// one row per platform (no “label”).
#[derive(Debug, Clone)]
pub struct PlatformConfig {
    pub platform_config_id: String,
    pub platform: String,
    pub client_id: Option<String>,
    pub client_secret: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl PlatformConfig {
    pub fn new(
        platform: &str,
        client_id: Option<&str>,
        client_secret: Option<&str>
    ) -> Self {
        let now = Utc::now();
        Self {
            platform_config_id: Uuid::new_v4().to_string(),
            platform: platform.to_string(),
            client_id: client_id.map(|s| s.to_string()),
            client_secret: client_secret.map(|s| s.to_string()),
            created_at: now,
            updated_at: now,
        }
    }
}

#[async_trait]
pub trait PlatformConfigRepository: Send + Sync {
    /// Insert or update the single config row for this `platform`.
    /// If a row already exists for `platform`, update it; otherwise insert new.
    async fn upsert_platform_config(
        &self,
        platform: &str,
        client_id: Option<String>,
        client_secret: Option<String>,
    ) -> Result<(), Error>;

    /// Retrieve by ID (not used as much now that each platform has just one).
    async fn get_platform_config(&self, platform_config_id: &str) -> Result<Option<PlatformConfig>, Error>;

    /// List all configs (optionally filtered by platform).
    async fn list_platform_configs(&self, maybe_platform: Option<&str>) -> Result<Vec<PlatformConfig>, Error>;

    /// Delete row by ID (or use `platform` if you prefer).
    async fn delete_platform_config(&self, platform_config_id: &str) -> Result<(), Error>;

    /// Retrieve the single row by platform (returns None if not found).
    async fn get_by_platform(&self, platform: &str) -> Result<Option<PlatformConfig>, Error>;

    /// Count how many rows exist for a given platform (should be 0 or 1 now).
    async fn count_for_platform(&self, platform: &str) -> Result<i64, Error>;
}

/// Postgres-based implementation.
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
        // We'll see if an existing row is present; if so, update. Otherwise, insert.
        let now = Utc::now();

        // Check existing
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
                .bind(&client_id)
                .bind(&client_secret)
                .bind(now)
                .bind(pc.platform_config_id)
                .execute(&self.pool)
                .await?;
            Ok(())
        } else {
            // insert
            let platform_config_id = Uuid::new_v4().to_string();
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
                .bind(&platform_config_id)
                .bind(platform)
                .bind(&client_id)
                .bind(&client_secret)
                .bind(now)
                .bind(now)
                .execute(&self.pool)
                .await?;
            Ok(())
        }
    }

    async fn get_platform_config(&self, platform_config_id: &str) -> Result<Option<PlatformConfig>, Error> {
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
            "#
        )
            .bind(platform_config_id)
            .fetch_optional(&self.pool)
            .await?;

        if let Some(r) = row {
            Ok(Some(PlatformConfig {
                platform_config_id: r.try_get("platform_config_id")?,
                platform: r.try_get("platform")?,
                client_id: r.try_get("client_id")?,
                client_secret: r.try_get("client_secret")?,
                created_at: r.try_get("created_at")?,
                updated_at: r.try_get("updated_at")?,
            }))
        } else {
            Ok(None)
        }
    }

    async fn list_platform_configs(&self, maybe_platform: Option<&str>) -> Result<Vec<PlatformConfig>, Error> {
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
                WHERE platform = $1
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

        let mut results = Vec::new();
        for row in rows {
            results.push(PlatformConfig {
                platform_config_id: row.try_get("platform_config_id")?,
                platform: row.try_get("platform")?,
                client_id: row.try_get("client_id")?,
                client_secret: row.try_get("client_secret")?,
                created_at: row.try_get("created_at")?,
                updated_at: row.try_get("updated_at")?,
            });
        }
        Ok(results)
    }

    async fn delete_platform_config(&self, platform_config_id: &str) -> Result<(), Error> {
        sqlx::query(
            r#"
            DELETE FROM platform_config
            WHERE platform_config_id = $1
            "#
        )
            .bind(platform_config_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn get_by_platform(&self, platform: &str) -> Result<Option<PlatformConfig>, Error> {
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
            WHERE platform = $1
            LIMIT 1
            "#
        )
            .bind(platform)
            .fetch_optional(&self.pool)
            .await?;

        if let Some(r) = row {
            Ok(Some(PlatformConfig {
                platform_config_id: r.try_get("platform_config_id")?,
                platform: r.try_get("platform")?,
                client_id: r.try_get("client_id")?,
                client_secret: r.try_get("client_secret")?,
                created_at: r.try_get("created_at")?,
                updated_at: r.try_get("updated_at")?,
            }))
        } else {
            Ok(None)
        }
    }

    async fn count_for_platform(&self, platform: &str) -> Result<i64, Error> {
        let row = sqlx::query(
            r#"
            SELECT COUNT(*) AS count
            FROM platform_config
            WHERE platform = $1
            "#
        )
            .bind(platform)
            .fetch_one(&self.pool)
            .await?;
        let count: i64 = row.try_get("count")?;
        Ok(count)
    }
}