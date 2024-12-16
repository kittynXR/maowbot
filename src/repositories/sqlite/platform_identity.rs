// src/repositories/sqlite/platform_identity.rs

use super::*;
use crate::models::{PlatformIdentity, Platform};
use crate::repositories::Repository;

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

        let record = sqlx::query!(
            r#"SELECT * FROM platform_identities
            WHERE platform = ? AND platform_user_id = ?"#,
            platform_str, platform_user_id
        )
            .fetch_optional(&self.pool)
            .await?;

        match record {
            Some(r) => {
                let platform = Platform::from(r.platform);
                let platform_roles: Vec<String> = serde_json::from_str(&r.platform_roles)?;
                let platform_data: serde_json::Value = serde_json::from_str(&r.platform_data)?;

                Ok(Some(PlatformIdentity {
                    platform_identity_id: r.platform_identity_id,
                    user_id: r.user_id,
                    platform,
                    platform_user_id: r.platform_user_id,
                    platform_username: r.platform_username,
                    platform_display_name: r.platform_display_name,
                    platform_roles,
                    platform_data,
                    created_at: r.created_at,
                    last_updated: r.last_updated,
                }))
            }
            None => Ok(None)
        }
    }

    pub async fn get_all_for_user(&self, user_id: &str)
                                  -> Result<Vec<PlatformIdentity>, Error> {
        let records = sqlx::query!(
            r#"SELECT * FROM platform_identities WHERE user_id = ?"#,
            user_id
        )
            .fetch_all(&self.pool)
            .await?;

        let mut identities = Vec::new();
        for r in records {
            let platform = Platform::from(r.platform);
            let platform_roles: Vec<String> = serde_json::from_str(&r.platform_roles)?;
            let platform_data: serde_json::Value = serde_json::from_str(&r.platform_data)?;

            identities.push(PlatformIdentity {
                platform_identity_id: r.platform_identity_id,
                user_id: r.user_id,
                platform,
                platform_user_id: r.platform_user_id,
                platform_username: r.platform_username,
                platform_display_name: r.platform_display_name,
                platform_roles,
                platform_data,
                created_at: r.created_at,
                last_updated: r.last_updated,
            });
        }

        Ok(identities)
    }
}

#[async_trait]
impl Repository<PlatformIdentity> for PlatformIdentityRepository {
    async fn create(&self, identity: &PlatformIdentity) -> Result<(), Error> {
        let platform_str = identity.platform.to_string();
        let roles_json = serde_json::to_string(&identity.platform_roles)?;
        let data_json = identity.platform_data.to_string();

        sqlx::query!(
            r#"INSERT INTO platform_identities (
                platform_identity_id, user_id, platform, platform_user_id,
                platform_username, platform_display_name, platform_roles,
                platform_data, created_at, last_updated
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
            identity.platform_identity_id,
            identity.user_id,
            platform_str,
            identity.platform_user_id,
            identity.platform_username,
            identity.platform_display_name,
            roles_json,
            data_json,
            identity.created_at,
            identity.last_updated
        )
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    async fn get(&self, id: &str) -> Result<Option<PlatformIdentity>, Error> {
        let record = sqlx::query!(
            r#"SELECT * FROM platform_identities WHERE platform_identity_id = ?"#,
            id
        )
            .fetch_optional(&self.pool)
            .await?;

        match record {
            Some(r) => {
                let platform = Platform::from(r.platform);
                let platform_roles: Vec<String> = serde_json::from_str(&r.platform_roles)?;
                let platform_data: serde_json::Value = serde_json::from_str(&r.platform_data)?;

                Ok(Some(PlatformIdentity {
                    platform_identity_id: r.platform_identity_id,
                    user_id: r.user_id,
                    platform,
                    platform_user_id: r.platform_user_id,
                    platform_username: r.platform_username,
                    platform_display_name: r.platform_display_name,
                    platform_roles,
                    platform_data,
                    created_at: r.created_at,
                    last_updated: r.last_updated,
                }))
            }
            None => Ok(None)
        }
    }

    async fn update(&self, identity: &PlatformIdentity) -> Result<(), Error> {
        let platform_str = identity.platform.to_string();
        let roles_json = serde_json::to_string(&identity.platform_roles)?;
        let data_json = identity.platform_data.to_string();

        sqlx::query!(
            r#"UPDATE platform_identities
            SET user_id = ?, platform = ?, platform_user_id = ?,
                platform_username = ?, platform_display_name = ?,
                platform_roles = ?, platform_data = ?, last_updated = ?
            WHERE platform_identity_id = ?"#,
            identity.user_id,
            platform_str,
            identity.platform_user_id,
            identity.platform_username,
            identity.platform_display_name,
            roles_json,
            data_json,
            identity.last_updated,
            identity.platform_identity_id
        )
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    async fn delete(&self, id: &str) -> Result<(), Error> {
        sqlx::query!(
            "DELETE FROM platform_identities WHERE platform_identity_id = ?",
            id
        )
            .execute(&self.pool)
            .await?;

        Ok(())
    }
}