// src/repositories/postgres/platform_identity.rs

use crate::models::{PlatformIdentity, Platform};
use crate::utils::time::{to_epoch, from_epoch};
use crate::Error;
use sqlx::{Pool, Postgres, Row};
use async_trait::async_trait;
use serde_json;

#[async_trait]
pub trait PlatformIdentityRepo {
    async fn create(&self, identity: &PlatformIdentity) -> Result<(), Error>;
    async fn get(&self, id: &str) -> Result<Option<PlatformIdentity>, Error>;
    async fn update(&self, identity: &PlatformIdentity) -> Result<(), Error>;
    async fn delete(&self, id: &str) -> Result<(), Error>;

    async fn get_by_platform(&self, platform: Platform, platform_user_id: &str)
                             -> Result<Option<PlatformIdentity>, Error>;

    async fn get_all_for_user(&self, user_id: &str)
                              -> Result<Vec<PlatformIdentity>, Error>;
}

pub struct PlatformIdentityRepository {
    pool: Pool<Postgres>,
}

impl PlatformIdentityRepository {
    pub fn new(pool: Pool<Postgres>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl PlatformIdentityRepo for PlatformIdentityRepository {
    async fn create(&self, identity: &PlatformIdentity) -> Result<(), Error> {
        let platform_str = identity.platform.to_string();
        let roles_json = serde_json::to_string(&identity.platform_roles)?;
        let data_json = identity.platform_data.to_string();

        sqlx::query(
            r#"
            INSERT INTO platform_identities (
                platform_identity_id, user_id, platform, platform_user_id,
                platform_username, platform_display_name, platform_roles,
                platform_data, created_at, last_updated
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            "#,
        )
            .bind(&identity.platform_identity_id)
            .bind(&identity.user_id)
            .bind(platform_str)
            .bind(&identity.platform_user_id)
            .bind(&identity.platform_username)
            .bind(&identity.platform_display_name)
            .bind(&roles_json)
            .bind(&data_json)
            .bind(to_epoch(identity.created_at))
            .bind(to_epoch(identity.last_updated))
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn get(&self, id: &str) -> Result<Option<PlatformIdentity>, Error> {
        let row = sqlx::query(
            r#"
            SELECT
                platform_identity_id,
                user_id,
                platform,
                platform_user_id,
                platform_username,
                platform_display_name,
                platform_roles,
                platform_data,
                created_at,
                last_updated
            FROM platform_identities
            WHERE platform_identity_id = $1
            "#,
        )
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;

        if let Some(r) = row {
            let roles_json: String = r.try_get("platform_roles")?;
            let data_json: String = r.try_get("platform_data")?;

            Ok(Some(PlatformIdentity {
                platform_identity_id: r.try_get("platform_identity_id")?,
                user_id: r.try_get("user_id")?,
                platform: Platform::from(r.try_get::<String, _>("platform")?),
                platform_user_id: r.try_get("platform_user_id")?,
                platform_username: r.try_get("platform_username")?,
                platform_display_name: r.try_get("platform_display_name")?,
                platform_roles: serde_json::from_str(&roles_json)?,
                platform_data: serde_json::from_str(&data_json)?,
                created_at: from_epoch(r.try_get::<i64, _>("created_at")?),
                last_updated: from_epoch(r.try_get::<i64, _>("last_updated")?),
            }))
        } else {
            Ok(None)
        }
    }

    async fn update(&self, identity: &PlatformIdentity) -> Result<(), Error> {
        let platform_str = identity.platform.to_string();
        let roles_json = serde_json::to_string(&identity.platform_roles)?;
        let data_json = identity.platform_data.to_string();

        sqlx::query(
            r#"
            UPDATE platform_identities
            SET user_id = $1,
                platform = $2,
                platform_user_id = $3,
                platform_username = $4,
                platform_display_name = $5,
                platform_roles = $6,
                platform_data = $7,
                last_updated = $8
            WHERE platform_identity_id = $9
            "#,
        )
            .bind(&identity.user_id)
            .bind(platform_str)
            .bind(&identity.platform_user_id)
            .bind(&identity.platform_username)
            .bind(&identity.platform_display_name)
            .bind(roles_json)
            .bind(data_json)
            .bind(to_epoch(identity.last_updated))
            .bind(&identity.platform_identity_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn delete(&self, id: &str) -> Result<(), Error> {
        sqlx::query("DELETE FROM platform_identities WHERE platform_identity_id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn get_by_platform(&self, platform: Platform, platform_user_id: &str)
                             -> Result<Option<PlatformIdentity>, Error>
    {
        let platform_str = platform.to_string();

        let row = sqlx::query(
            r#"
            SELECT
                platform_identity_id,
                user_id,
                platform,
                platform_user_id,
                platform_username,
                platform_display_name,
                platform_roles,
                platform_data,
                created_at,
                last_updated
            FROM platform_identities
            WHERE platform = $1 AND platform_user_id = $2
            "#,
        )
            .bind(platform_str)
            .bind(platform_user_id)
            .fetch_optional(&self.pool)
            .await?;

        if let Some(r) = row {
            let roles_json: String = r.try_get("platform_roles")?;
            let data_json: String = r.try_get("platform_data")?;

            Ok(Some(PlatformIdentity {
                platform_identity_id: r.try_get("platform_identity_id")?,
                user_id: r.try_get("user_id")?,
                platform: Platform::from(r.try_get::<String, _>("platform")?),
                platform_user_id: r.try_get("platform_user_id")?,
                platform_username: r.try_get("platform_username")?,
                platform_display_name: r.try_get("platform_display_name")?,
                platform_roles: serde_json::from_str(&roles_json)?,
                platform_data: serde_json::from_str(&data_json)?,
                created_at: from_epoch(r.try_get::<i64, _>("created_at")?),
                last_updated: from_epoch(r.try_get::<i64, _>("last_updated")?),
            }))
        } else {
            Ok(None)
        }
    }

    async fn get_all_for_user(&self, user_id: &str)
                              -> Result<Vec<PlatformIdentity>, Error>
    {
        let rows = sqlx::query(
            r#"
            SELECT
                platform_identity_id,
                user_id,
                platform,
                platform_user_id,
                platform_username,
                platform_display_name,
                platform_roles,
                platform_data,
                created_at,
                last_updated
            FROM platform_identities
            WHERE user_id = $1
            "#,
        )
            .bind(user_id)
            .fetch_all(&self.pool)
            .await?;

        let mut identities = Vec::new();
        for r in rows {
            let roles_json: String = r.try_get("platform_roles")?;
            let data_json: String = r.try_get("platform_data")?;

            identities.push(PlatformIdentity {
                platform_identity_id: r.try_get("platform_identity_id")?,
                user_id: r.try_get("user_id")?,
                platform: Platform::from(r.try_get::<String, _>("platform")?),
                platform_user_id: r.try_get("platform_user_id")?,
                platform_username: r.try_get("platform_username")?,
                platform_display_name: r.try_get("platform_display_name")?,
                platform_roles: serde_json::from_str(&roles_json)?,
                platform_data: serde_json::from_str(&data_json)?,
                created_at: from_epoch(r.try_get::<i64, _>("created_at")?),
                last_updated: from_epoch(r.try_get::<i64, _>("last_updated")?),
            });
        }

        Ok(identities)
    }
}