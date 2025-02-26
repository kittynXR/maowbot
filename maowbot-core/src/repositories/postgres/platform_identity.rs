use crate::models::{PlatformIdentity, Platform};
use crate::Error;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde_json::Value;
use sqlx::{Pool, Postgres, Row};
use uuid::Uuid;

#[async_trait]
pub trait PlatformIdentityRepo {
    async fn create(&self, identity: &PlatformIdentity) -> Result<(), Error>;
    async fn get(&self, id: Uuid) -> Result<Option<PlatformIdentity>, Error>;
    async fn update(&self, identity: &PlatformIdentity) -> Result<(), Error>;
    async fn delete(&self, id: Uuid) -> Result<(), Error>;

    async fn get_by_platform(
        &self,
        platform: Platform,
        platform_user_id: &str
    ) -> Result<Option<PlatformIdentity>, Error>;

    async fn get_all_for_user(&self, user_id: Uuid)
                              -> Result<Vec<PlatformIdentity>, Error>;

    async fn get_by_user_and_platform(
        &self,
        user_id: Uuid,
        platform: &Platform,
    ) -> Result<Option<PlatformIdentity>, Error>;
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

        // We'll store platform_roles as JSONB with ::jsonb,
        // so we must pass a JSON string. Similarly for platform_data.
        let roles_json = serde_json::to_string(&identity.platform_roles)?;
        let data_json  = identity.platform_data.to_string(); // already a Value, so just `.to_string()`

        sqlx::query(
            r#"
            INSERT INTO platform_identities (
                platform_identity_id, user_id, platform, platform_user_id,
                platform_username, platform_display_name,
                platform_roles,         -- stored as jsonb
                platform_data,          -- stored as jsonb
                created_at, last_updated
            )
            VALUES (
                $1, $2, $3, $4,
                $5, $6,
                $7::jsonb, $8::jsonb,
                $9, $10
            )
            "#,
        )
            .bind(identity.platform_identity_id)
            .bind(identity.user_id)
            .bind(platform_str)
            .bind(&identity.platform_user_id)
            .bind(&identity.platform_username)
            .bind(&identity.platform_display_name)
            .bind(roles_json) // cast to jsonb
            .bind(data_json)  // cast to jsonb
            .bind(identity.created_at)
            .bind(identity.last_updated)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    async fn get(&self, id: Uuid) -> Result<Option<PlatformIdentity>, Error> {
        let row = sqlx::query(
            r#"
            SELECT
                platform_identity_id, user_id, platform, platform_user_id,
                platform_username, platform_display_name,
                platform_roles,     -- jsonb
                platform_data,      -- jsonb
                created_at, last_updated
            FROM platform_identities
            WHERE platform_identity_id = $1
            "#,
        )
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;

        if let Some(r) = row {
            // Instead of a String, decode JSONB directly into serde_json::Value
            let roles_val: Value = r.try_get("platform_roles")?;
            let data_val: Value  = r.try_get("platform_data")?;

            // If you want `platform_roles` as Vec<String>, parse from Value
            let roles_vec: Vec<String> = serde_json::from_value(roles_val)?;

            let pi = PlatformIdentity {
                platform_identity_id: r.try_get("platform_identity_id")?,
                user_id: r.try_get("user_id")?,
                platform: Platform::from(r.try_get::<String, _>("platform")?),
                platform_user_id: r.try_get("platform_user_id")?,
                platform_username: r.try_get("platform_username")?,
                platform_display_name: r.try_get("platform_display_name")?,
                platform_roles: roles_vec,
                platform_data: data_val,
                created_at: r.try_get("created_at")?,
                last_updated: r.try_get("last_updated")?,
            };
            Ok(Some(pi))
        } else {
            Ok(None)
        }
    }

    async fn update(&self, identity: &PlatformIdentity) -> Result<(), Error> {
        let platform_str = identity.platform.to_string();
        let roles_json   = serde_json::to_string(&identity.platform_roles)?;
        let data_json    = identity.platform_data.to_string();

        sqlx::query(
            r#"
            UPDATE platform_identities
            SET user_id               = $1,
                platform              = $2,
                platform_user_id      = $3,
                platform_username     = $4,
                platform_display_name = $5,
                platform_roles        = $6::jsonb,
                platform_data         = $7::jsonb,
                last_updated          = $8
            WHERE platform_identity_id = $9
            "#,
        )
            .bind(identity.user_id)
            .bind(platform_str)
            .bind(&identity.platform_user_id)
            .bind(&identity.platform_username)
            .bind(&identity.platform_display_name)
            .bind(roles_json) // cast to jsonb
            .bind(data_json)  // cast to jsonb
            .bind(identity.last_updated)
            .bind(identity.platform_identity_id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    async fn delete(&self, id: Uuid) -> Result<(), Error> {
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
                platform_identity_id, user_id, platform, platform_user_id,
                platform_username, platform_display_name,
                platform_roles,     -- jsonb
                platform_data,      -- jsonb
                created_at, last_updated
            FROM platform_identities
            WHERE platform = $1 AND platform_user_id = $2
            "#,
        )
            .bind(platform_str)
            .bind(platform_user_id)
            .fetch_optional(&self.pool)
            .await?;

        if let Some(r) = row {
            let roles_val: Value = r.try_get("platform_roles")?;
            let data_val: Value  = r.try_get("platform_data")?;
            let roles_vec: Vec<String> = serde_json::from_value(roles_val)?;

            let pi = PlatformIdentity {
                platform_identity_id: r.try_get("platform_identity_id")?,
                user_id: r.try_get("user_id")?,
                platform: Platform::from(r.try_get::<String, _>("platform")?),
                platform_user_id: r.try_get("platform_user_id")?,
                platform_username: r.try_get("platform_username")?,
                platform_display_name: r.try_get("platform_display_name")?,
                platform_roles: roles_vec,
                platform_data: data_val,
                created_at: r.try_get("created_at")?,
                last_updated: r.try_get("last_updated")?,
            };
            Ok(Some(pi))
        } else {
            Ok(None)
        }
    }

    async fn get_all_for_user(&self, user_id: Uuid)
                              -> Result<Vec<PlatformIdentity>, Error>
    {
        let rows = sqlx::query(
            r#"
            SELECT
                platform_identity_id, user_id, platform, platform_user_id,
                platform_username, platform_display_name,
                platform_roles,     -- jsonb
                platform_data,      -- jsonb
                created_at, last_updated
            FROM platform_identities
            WHERE user_id = $1
            "#,
        )
            .bind(user_id)
            .fetch_all(&self.pool)
            .await?;

        let mut identities = Vec::new();
        for r in rows {
            let roles_val: Value = r.try_get("platform_roles")?;
            let data_val: Value  = r.try_get("platform_data")?;
            let roles_vec: Vec<String> = serde_json::from_value(roles_val)?;

            let pi = PlatformIdentity {
                platform_identity_id: r.try_get("platform_identity_id")?,
                user_id: r.try_get("user_id")?,
                platform: Platform::from(r.try_get::<String, _>("platform")?),
                platform_user_id: r.try_get("platform_user_id")?,
                platform_username: r.try_get("platform_username")?,
                platform_display_name: r.try_get("platform_display_name")?,
                platform_roles: roles_vec,
                platform_data: data_val,
                created_at: r.try_get("created_at")?,
                last_updated: r.try_get("last_updated")?,
            };
            identities.push(pi);
        }
        Ok(identities)
    }

    async fn get_by_user_and_platform(
        &self,
        user_id: Uuid,
        platform: &Platform,
    ) -> Result<Option<PlatformIdentity>, Error> {
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
                platform_roles,     -- jsonb
                platform_data,      -- jsonb
                created_at,
                last_updated
            FROM platform_identities
            WHERE user_id = $1
              AND platform = $2
            LIMIT 1
            "#,
        )
            .bind(user_id)
            .bind(platform_str)
            .fetch_optional(&self.pool)
            .await?;

        if let Some(r) = row {
            let roles_val: serde_json::Value = r.try_get("platform_roles")?;
            let data_val: serde_json::Value  = r.try_get("platform_data")?;
            let roles_vec: Vec<String> = serde_json::from_value(roles_val)?;

            let pi = PlatformIdentity {
                platform_identity_id: r.try_get("platform_identity_id")?,
                user_id: r.try_get("user_id")?,
                platform: Platform::from(r.try_get::<String, _>("platform")?),
                platform_user_id: r.try_get("platform_user_id")?,
                platform_username: r.try_get("platform_username")?,
                platform_display_name: r.try_get("platform_display_name")?,
                platform_roles: roles_vec,
                platform_data: data_val,
                created_at: r.try_get("created_at")?,
                last_updated: r.try_get("last_updated")?,
            };
            Ok(Some(pi))
        } else {
            Ok(None)
        }
    }
}
