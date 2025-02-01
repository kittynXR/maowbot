// src/repositories/sqlite/platform_identity.rs

use super::*;
use crate::models::{PlatformIdentity, Platform};
use crate::repositories::Repository;
use crate::Error;
use sqlx::{Pool, Sqlite, Row};
use serde_json;
use crate::utils::time::{to_epoch, from_epoch};

pub struct PlatformIdentityRepository {
    pool: Pool<Sqlite>
}

impl PlatformIdentityRepository {
    pub fn new(pool: Pool<Sqlite>) -> Self {
        Self { pool }
    }

    pub async fn get_by_platform(&self, platform: Platform, platform_user_id: &str)
                                 -> Result<Option<PlatformIdentity>, Error> {
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
            WHERE platform = ? AND platform_user_id = ?
            "#
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

    pub async fn get_all_for_user(&self, user_id: &str)
                                  -> Result<Vec<PlatformIdentity>, Error> {
        let records = sqlx::query(
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
            WHERE user_id = ?
            "#
        )
            .bind(user_id)
            .fetch_all(&self.pool)
            .await?;

        let mut identities = Vec::new();
        for r in records {
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

#[async_trait::async_trait]
impl Repository<PlatformIdentity> for PlatformIdentityRepository {
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
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#
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
            WHERE platform_identity_id = ?
            "#
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
            SET user_id = ?,
                platform = ?,
                platform_user_id = ?,
                platform_username = ?,
                platform_display_name = ?,
                platform_roles = ?,
                platform_data = ?,
                last_updated = ?
            WHERE platform_identity_id = ?
            "#
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
        sqlx::query("DELETE FROM platform_identities WHERE platform_identity_id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}
