// maowbot-core/src/repositories/postgres/platform_config.rs


use sqlx::{Pool, Postgres, Row};
use async_trait::async_trait;
use uuid::Uuid;
use chrono::{DateTime, Utc};
use crate::Error;

/// Basic struct representing a row in `platform_config`, storing
/// client_id/secret for a particular platform (and label).
#[derive(Debug, Clone)]
pub struct PlatformConfig {
    pub platform_config_id: String,
    pub platform: String,
    pub platform_label: Option<String>,
    pub client_id: Option<String>,
    pub client_secret: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl PlatformConfig {
    pub fn new(
        platform: &str,
        platform_label: Option<&str>,
        client_id: Option<&str>,
        client_secret: Option<&str>
    ) -> Self {
        let now = Utc::now();
        Self {
            platform_config_id: Uuid::new_v4().to_string(),
            platform: platform.to_string(),
            platform_label: platform_label.map(|s| s.to_string()),
            client_id: client_id.map(|s| s.to_string()),
            client_secret: client_secret.map(|s| s.to_string()),
            created_at: now,
            updated_at: now,
        }
    }
}

#[async_trait]
pub trait PlatformConfigRepository: Send + Sync {
    /// Insert a new platform_config row.
    async fn insert_platform_config(
        &self,
        platform: &str,
        label: &str,
        client_id: String,
        client_secret: Option<String>,
    ) -> Result<(), Error>;

    /// Retrieve by ID.
    async fn get_platform_config(&self, platform_config_id: &str) -> Result<Option<PlatformConfig>, Error>;

    /// List all rows (optionally filtering by platform).
    async fn list_platform_configs(&self, maybe_platform: Option<&str>) -> Result<Vec<PlatformConfig>, Error>;

    /// Update existing row (by ID).
    async fn update_platform_config(&self, config: &PlatformConfig) -> Result<(), Error>;

    /// Delete row by ID.
    async fn delete_platform_config(&self, platform_config_id: &str) -> Result<(), Error>;

    /// Retrieve a single row by (platform, label).
    async fn get_by_platform_and_label(&self, platform: &str, label: &str) -> Result<Option<PlatformConfig>, Error>;

    /// Count how many rows exist for a given platform.
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
    async fn insert_platform_config(
        &self,
        platform: &str,
        label: &str,
        client_id: String,
        client_secret: Option<String>,
    ) -> Result<(), Error> {
        let now = Utc::now();
        let platform_config_id = Uuid::new_v4().to_string();
        sqlx::query(
            r#"
            INSERT INTO platform_config (
                platform_config_id,
                platform,
                platform_label,
                client_id,
                client_secret,
                created_at,
                updated_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            "#
        )
            .bind(&platform_config_id)
            .bind(platform)
            .bind(label)
            .bind(&client_id)
            .bind(&client_secret)
            .bind(now)
            .bind(now)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn get_platform_config(&self, platform_config_id: &str) -> Result<Option<PlatformConfig>, Error> {
        let row = sqlx::query(
            r#"
            SELECT
                platform_config_id,
                platform,
                platform_label,
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
                platform_label: r.try_get("platform_label")?,
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
                    platform_label,
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
                    platform_label,
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
                platform_label: row.try_get("platform_label")?,
                client_id: row.try_get("client_id")?,
                client_secret: row.try_get("client_secret")?,
                created_at: row.try_get("created_at")?,
                updated_at: row.try_get("updated_at")?,
            });
        }
        Ok(results)
    }

    async fn update_platform_config(&self, config: &PlatformConfig) -> Result<(), Error> {
        let now = Utc::now();
        sqlx::query(
            r#"
            UPDATE platform_config
            SET platform = $1,
                platform_label = $2,
                client_id = $3,
                client_secret = $4,
                updated_at = $5
            WHERE platform_config_id = $6
            "#
        )
            .bind(&config.platform)
            .bind(&config.platform_label)
            .bind(&config.client_id)
            .bind(&config.client_secret)
            .bind(now)
            .bind(&config.platform_config_id)
            .execute(&self.pool)
            .await?;
        Ok(())
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

    async fn get_by_platform_and_label(&self, platform: &str, label: &str) -> Result<Option<PlatformConfig>, Error> {
        let row = sqlx::query(
            r#"
            SELECT
                platform_config_id,
                platform,
                platform_label,
                client_id,
                client_secret,
                created_at,
                updated_at
            FROM platform_config
            WHERE platform = $1
              AND platform_label = $2
            LIMIT 1
            "#
        )
            .bind(platform)
            .bind(label)
            .fetch_optional(&self.pool)
            .await?;

        if let Some(r) = row {
            Ok(Some(PlatformConfig {
                platform_config_id: r.try_get("platform_config_id")?,
                platform: r.try_get("platform")?,
                platform_label: r.try_get("platform_label")?,
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