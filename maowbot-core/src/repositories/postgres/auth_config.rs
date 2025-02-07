use sqlx::{Pool, Postgres, Row};
use async_trait::async_trait;
use uuid::Uuid;
use chrono::{DateTime, Utc};
use crate::Error;

/// Basic struct representing a row in `auth_config`, storing
/// client_id/secret for a particular platform or app name.
#[derive(Debug, Clone)]
pub struct AuthConfig {
    pub auth_config_id: String,
    pub platform: String,
    pub app_label: Option<String>,
    pub client_id: Option<String>,
    pub client_secret: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl AuthConfig {
    pub fn new(platform: &str, app_label: Option<&str>, client_id: Option<&str>, client_secret: Option<&str>) -> Self {
        let now = Utc::now();
        Self {
            auth_config_id: Uuid::new_v4().to_string(),
            platform: platform.to_string(),
            app_label: app_label.map(|s| s.to_string()),
            client_id: client_id.map(|s| s.to_string()),
            client_secret: client_secret.map(|s| s.to_string()),
            created_at: now,
            updated_at: now,
        }
    }
}

#[async_trait]
pub trait AuthConfigRepository: Send + Sync {
    /// Insert a new auth config row.
    async fn create_auth_config(&self, config: &AuthConfig) -> Result<(), Error>;

    /// Retrieve a row by ID.
    async fn get_auth_config(&self, auth_config_id: &str) -> Result<Option<AuthConfig>, Error>;

    /// Retrieve all rows for a given platform (or all if platform is None).
    async fn list_auth_configs(&self, maybe_platform: Option<&str>) -> Result<Vec<AuthConfig>, Error>;

    /// Update an existing row (by ID).
    async fn update_auth_config(&self, config: &AuthConfig) -> Result<(), Error>;

    /// Delete by ID.
    async fn delete_auth_config(&self, auth_config_id: &str) -> Result<(), Error>;
}

/// Postgres-based impl.
#[derive(Clone)]
pub struct PostgresAuthConfigRepository {
    pool: Pool<Postgres>,
}

impl PostgresAuthConfigRepository {
    pub fn new(pool: Pool<Postgres>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl AuthConfigRepository for PostgresAuthConfigRepository {
    async fn create_auth_config(&self, config: &AuthConfig) -> Result<(), Error> {
        sqlx::query(
            r#"
            INSERT INTO auth_config (
                auth_config_id,
                platform,
                app_label,
                client_id,
                client_secret,
                created_at,
                updated_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            "#
        )
            .bind(&config.auth_config_id)
            .bind(&config.platform)
            .bind(&config.app_label)
            .bind(&config.client_id)
            .bind(&config.client_secret)
            .bind(config.created_at)
            .bind(config.updated_at)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    async fn get_auth_config(&self, auth_config_id: &str) -> Result<Option<AuthConfig>, Error> {
        let row = sqlx::query(
            r#"
            SELECT
                auth_config_id,
                platform,
                app_label,
                client_id,
                client_secret,
                created_at,
                updated_at
            FROM auth_config
            WHERE auth_config_id = $1
            "#,
        )
            .bind(auth_config_id)
            .fetch_optional(&self.pool)
            .await?;

        if let Some(r) = row {
            Ok(Some(AuthConfig {
                auth_config_id: r.try_get("auth_config_id")?,
                platform: r.try_get("platform")?,
                app_label: r.try_get("app_label")?,
                client_id: r.try_get("client_id")?,
                client_secret: r.try_get("client_secret")?,
                created_at: r.try_get("created_at")?,
                updated_at: r.try_get("updated_at")?,
            }))
        } else {
            Ok(None)
        }
    }

    async fn list_auth_configs(&self, maybe_platform: Option<&str>) -> Result<Vec<AuthConfig>, Error> {
        if let Some(p) = maybe_platform {
            let rows = sqlx::query(
                r#"
                SELECT
                    auth_config_id,
                    platform,
                    app_label,
                    client_id,
                    client_secret,
                    created_at,
                    updated_at
                FROM auth_config
                WHERE platform = $1
                ORDER BY created_at DESC
                "#
            )
                .bind(p)
                .fetch_all(&self.pool)
                .await?;

            let mut results = Vec::new();
            for row in rows {
                results.push(AuthConfig {
                    auth_config_id: row.try_get("auth_config_id")?,
                    platform: row.try_get("platform")?,
                    app_label: row.try_get("app_label")?,
                    client_id: row.try_get("client_id")?,
                    client_secret: row.try_get("client_secret")?,
                    created_at: row.try_get("created_at")?,
                    updated_at: row.try_get("updated_at")?,
                });
            }
            Ok(results)
        } else {
            let rows = sqlx::query(
                r#"
                SELECT
                    auth_config_id,
                    platform,
                    app_label,
                    client_id,
                    client_secret,
                    created_at,
                    updated_at
                FROM auth_config
                ORDER BY created_at DESC
                "#
            )
                .fetch_all(&self.pool)
                .await?;

            let mut results = Vec::new();
            for row in rows {
                results.push(AuthConfig {
                    auth_config_id: row.try_get("auth_config_id")?,
                    platform: row.try_get("platform")?,
                    app_label: row.try_get("app_label")?,
                    client_id: row.try_get("client_id")?,
                    client_secret: row.try_get("client_secret")?,
                    created_at: row.try_get("created_at")?,
                    updated_at: row.try_get("updated_at")?,
                });
            }
            Ok(results)
        }
    }

    async fn update_auth_config(&self, config: &AuthConfig) -> Result<(), Error> {
        let now = Utc::now();
        sqlx::query(
            r#"
            UPDATE auth_config
            SET platform = $1,
                app_label = $2,
                client_id = $3,
                client_secret = $4,
                updated_at = $5
            WHERE auth_config_id = $6
            "#,
        )
            .bind(&config.platform)
            .bind(&config.app_label)
            .bind(&config.client_id)
            .bind(&config.client_secret)
            .bind(now)
            .bind(&config.auth_config_id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    async fn delete_auth_config(&self, auth_config_id: &str) -> Result<(), Error> {
        sqlx::query(
            r#"
            DELETE FROM auth_config
            WHERE auth_config_id = $1
            "#,
        )
            .bind(auth_config_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}
