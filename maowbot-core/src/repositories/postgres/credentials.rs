use crate::{Error, crypto::Encryptor};
use async_trait::async_trait;
use chrono::{Utc, Duration};
use sqlx::{Pool, Postgres, Row};
use std::str::FromStr;
use uuid::Uuid;
use maowbot_common::models::platform::{Platform, PlatformCredential};
pub(crate) use maowbot_common::traits::repository_traits::CredentialsRepository;

#[derive(Clone)]
pub struct PostgresCredentialsRepository {
    pool: Pool<Postgres>,
    encryptor: Encryptor,
}

impl PostgresCredentialsRepository {
    pub fn new(pool: Pool<Postgres>, encryptor: Encryptor) -> Self {
        Self { pool, encryptor }
    }
}

#[async_trait]
impl CredentialsRepository for PostgresCredentialsRepository {
    async fn store_credentials(&self, creds: &PlatformCredential) -> Result<(), Error> {
        let platform_str = creds.platform.to_string();
        let cred_type_str = creds.credential_type.to_string();

        // Encrypt sensitive fields
        let encrypted_token = self.encryptor.encrypt(&creds.primary_token)?;
        let encrypted_refresh = match &creds.refresh_token {
            Some(token) => Some(self.encryptor.encrypt(token)?),
            None => None,
        };
        let encrypted_data = match &creds.additional_data {
            Some(data) => Some(self.encryptor.encrypt(&data.to_string())?),
            None => None,
        };

        sqlx::query(
            r#"
            INSERT INTO platform_credentials (
                credential_id,
                platform,
                platform_id,
                credential_type,
                user_id,
                user_name,
                primary_token,
                refresh_token,
                additional_data,
                expires_at,
                created_at,
                updated_at,
                is_bot
            )
            VALUES ($1, $2, $3, $4, $5, $6,
                    $7, $8, $9, $10, $11, $12, $13)
            ON CONFLICT (platform, user_id) DO UPDATE
               SET
                 platform_id     = EXCLUDED.platform_id,
                 user_name       = EXCLUDED.user_name,
                 primary_token   = EXCLUDED.primary_token,
                 refresh_token   = EXCLUDED.refresh_token,
                 additional_data = EXCLUDED.additional_data,
                 expires_at      = EXCLUDED.expires_at,
                 updated_at      = EXCLUDED.updated_at,
                 is_bot          = EXCLUDED.is_bot
            "#,
        )
            .bind(creds.credential_id)
            .bind(platform_str)
            .bind(&creds.platform_id)
            .bind(cred_type_str)
            .bind(creds.user_id)
            .bind(&creds.user_name)
            .bind(encrypted_token)
            .bind(encrypted_refresh)
            .bind(encrypted_data)
            .bind(creds.expires_at)
            .bind(creds.created_at)
            .bind(creds.updated_at)
            .bind(creds.is_bot)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    async fn get_credentials(&self, platform: &Platform, user_id: Uuid) -> Result<Option<PlatformCredential>, Error> {
        let row_opt = sqlx::query(
            r#"
            SELECT
                credential_id,
                platform,
                platform_id,
                credential_type,
                user_id,
                user_name,
                primary_token,
                refresh_token,
                additional_data,
                expires_at,
                created_at,
                updated_at,
                is_bot
            FROM platform_credentials
            WHERE LOWER(platform) = LOWER($1)
              AND user_id = $2
            "#,
        )
            .bind(platform.to_string())
            .bind(user_id)
            .fetch_optional(&self.pool)
            .await?;

        if let Some(r) = row_opt {
            let decrypted_token = self.encryptor.decrypt(r.try_get("primary_token")?)?;
            let ref_opt: Option<String> = r.try_get("refresh_token")?;
            let decrypted_refresh = if let Some(s) = ref_opt {
                Some(self.encryptor.decrypt(&s)?)
            } else {
                None
            };
            let data_opt: Option<String> = r.try_get("additional_data")?;
            let decrypted_data = if let Some(encrypted_data) = data_opt {
                let json_str = self.encryptor.decrypt(&encrypted_data)?;
                Some(serde_json::from_str(&json_str)?)
            } else {
                None
            };

            let pc = PlatformCredential {
                credential_id: r.try_get("credential_id")?,
                platform: Platform::from_str(&r.try_get::<String, _>("platform")?)
                    .map_err(|e| Error::Platform(e.to_string()))?,
                platform_id: r.try_get("platform_id")?,
                credential_type: r.try_get::<String, _>("credential_type")?.parse()?,
                user_id: r.try_get("user_id")?,
                user_name: r.try_get("user_name")?,
                primary_token: decrypted_token,
                refresh_token: decrypted_refresh,
                additional_data: decrypted_data,
                expires_at: r.try_get("expires_at")?,
                created_at: r.try_get("created_at")?,
                updated_at: r.try_get("updated_at")?,
                is_bot: r.try_get("is_bot")?,
            };
            Ok(Some(pc))
        } else {
            Ok(None)
        }
    }

    async fn get_credential_by_id(&self, credential_id: Uuid) -> Result<Option<PlatformCredential>, Error> {
        // For brevity, let's do a manual SELECT + decrypt like in the other methods:
        let row_opt = sqlx::query(
            r#"
            SELECT
                credential_id,
                platform,
                platform_id,
                credential_type,
                user_id,
                user_name,
                primary_token,
                refresh_token,
                additional_data,
                expires_at,
                created_at,
                updated_at,
                is_bot
            FROM platform_credentials
            WHERE credential_id = $1
            LIMIT 1
            "#,
        )
            .bind(credential_id)
            .fetch_optional(&self.pool)
            .await?;

        if let Some(r) = row_opt {
            let dec_token = self.encryptor.decrypt(r.try_get("primary_token")?)?;
            let rfr_opt: Option<String> = r.try_get("refresh_token")?;
            let dec_refresh = if let Some(encrypted_r) = rfr_opt {
                Some(self.encryptor.decrypt(&encrypted_r)?)
            } else {
                None
            };
            let data_opt: Option<String> = r.try_get("additional_data")?;
            let dec_data = if let Some(d) = data_opt {
                let js = self.encryptor.decrypt(&d)?;
                Some(serde_json::from_str(&js)?)
            } else {
                None
            };

            let pc = PlatformCredential {
                credential_id: r.try_get("credential_id")?,
                platform: Platform::from_str(&r.try_get::<String, _>("platform")?)
                    .map_err(|e| Error::Platform(e.to_string()))?,
                platform_id: r.try_get("platform_id")?,
                credential_type: r.try_get::<String, _>("credential_type")?.parse()?,
                user_id: r.try_get("user_id")?,
                user_name: r.try_get("user_name")?,
                primary_token: dec_token,
                refresh_token: dec_refresh,
                additional_data: dec_data,
                expires_at: r.try_get("expires_at")?,
                created_at: r.try_get("created_at")?,
                updated_at: r.try_get("updated_at")?,
                is_bot: r.try_get("is_bot")?,
            };
            Ok(Some(pc))
        } else {
            Ok(None)
        }
    }

    async fn update_credentials(&self, creds: &PlatformCredential) -> Result<(), Error> {
        let platform_str = creds.platform.to_string();

        let encrypted_token = self.encryptor.encrypt(&creds.primary_token)?;
        let encrypted_refresh = match &creds.refresh_token {
            Some(r) => Some(self.encryptor.encrypt(r)?),
            None => None,
        };
        let encrypted_data = match &creds.additional_data {
            Some(d) => Some(self.encryptor.encrypt(&d.to_string())?),
            None => None,
        };

        sqlx::query(
            r#"
            UPDATE platform_credentials
            SET
              platform_id     = $1,
              user_name       = $2,
              primary_token   = $3,
              refresh_token   = $4,
              additional_data = $5,
              expires_at      = $6,
              updated_at      = $7,
              is_bot          = $8
            WHERE LOWER(platform) = LOWER($9)
              AND user_id = $10
            "#,
        )
            .bind(&creds.platform_id)
            .bind(&creds.user_name)
            .bind(encrypted_token)
            .bind(encrypted_refresh)
            .bind(encrypted_data)
            .bind(creds.expires_at)
            .bind(creds.updated_at)
            .bind(creds.is_bot)
            .bind(platform_str)
            .bind(creds.user_id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    async fn delete_credentials(&self, platform: &Platform, user_id: Uuid) -> Result<(), Error> {
        sqlx::query(
            r#"
            DELETE FROM platform_credentials
            WHERE LOWER(platform) = LOWER($1)
              AND user_id = $2
            "#
        )
            .bind(platform.to_string())
            .bind(user_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn get_expiring_credentials(&self, within: Duration) -> Result<Vec<PlatformCredential>, Error> {
        let cutoff = Utc::now() + within;
        let rows = sqlx::query(
            r#"
            SELECT
                credential_id,
                platform,
                platform_id,
                credential_type,
                user_id,
                user_name,
                primary_token,
                refresh_token,
                additional_data,
                expires_at,
                created_at,
                updated_at,
                is_bot
            FROM platform_credentials
            WHERE expires_at IS NOT NULL
              AND expires_at <= $1
            "#
        )
            .bind(cutoff)
            .fetch_all(&self.pool)
            .await?;

        let mut results = Vec::new();
        for r in rows {
            let dec_token = self.encryptor.decrypt(r.try_get("primary_token")?)?;
            let rfr_opt: Option<String> = r.try_get("refresh_token")?;
            let dec_refresh = if let Some(rx) = rfr_opt {
                Some(self.encryptor.decrypt(&rx)?)
            } else {
                None
            };
            let data_opt: Option<String> = r.try_get("additional_data")?;
            let dec_data = if let Some(d) = data_opt {
                let js = self.encryptor.decrypt(&d)?;
                Some(serde_json::from_str(&js)?)
            } else {
                None
            };

            results.push(PlatformCredential {
                credential_id: r.try_get("credential_id")?,
                platform: r.try_get::<String, _>("platform")?.parse()?,
                platform_id: r.try_get("platform_id")?,
                credential_type: r.try_get::<String, _>("credential_type")?.parse()?,
                user_id: r.try_get("user_id")?,
                user_name: r.try_get("user_name")?,
                primary_token: dec_token,
                refresh_token: dec_refresh,
                additional_data: dec_data,
                expires_at: r.try_get("expires_at")?,
                created_at: r.try_get("created_at")?,
                updated_at: r.try_get("updated_at")?,
                is_bot: r.try_get("is_bot")?,
            });
        }
        Ok(results)
    }

    async fn get_all_credentials(&self) -> Result<Vec<PlatformCredential>, Error> {
        let rows = sqlx::query(
            r#"
            SELECT
                credential_id,
                platform,
                platform_id,
                credential_type,
                user_id,
                user_name,
                primary_token,
                refresh_token,
                additional_data,
                expires_at,
                created_at,
                updated_at,
                is_bot
            FROM platform_credentials
            "#
        )
            .fetch_all(&self.pool)
            .await?;

        let mut creds = Vec::new();
        for r in rows {
            let dec_token = self.encryptor.decrypt(r.try_get("primary_token")?)?;
            let ref_opt: Option<String> = r.try_get("refresh_token")?;
            let dec_refresh = if let Some(rr) = ref_opt {
                Some(self.encryptor.decrypt(&rr)?)
            } else {
                None
            };
            let data_opt: Option<String> = r.try_get("additional_data")?;
            let dec_data = if let Some(j) = data_opt {
                let js = self.encryptor.decrypt(&j)?;
                Some(serde_json::from_str(&js)?)
            } else {
                None
            };

            creds.push(PlatformCredential {
                credential_id: r.try_get("credential_id")?,
                platform: r.try_get::<String, _>("platform")?.parse()?,
                platform_id: r.try_get("platform_id")?,
                credential_type: r.try_get::<String, _>("credential_type")?.parse()?,
                user_id: r.try_get("user_id")?,
                user_name: r.try_get("user_name")?,
                primary_token: dec_token,
                refresh_token: dec_refresh,
                additional_data: dec_data,
                expires_at: r.try_get("expires_at")?,
                created_at: r.try_get("created_at")?,
                updated_at: r.try_get("updated_at")?,
                is_bot: r.try_get("is_bot")?,
            });
        }
        Ok(creds)
    }

    /// **NEW** method to list credentials for exactly one `platform`.
    async fn list_credentials_for_platform(&self, platform: &Platform) -> Result<Vec<PlatformCredential>, Error> {
        let rows = sqlx::query(
            r#"
            SELECT
                credential_id,
                platform,
                platform_id,
                credential_type,
                user_id,
                user_name,
                primary_token,
                refresh_token,
                additional_data,
                expires_at,
                created_at,
                updated_at,
                is_bot
            FROM platform_credentials
            WHERE LOWER(platform) = LOWER($1)
            "#,
        )
            .bind(platform.to_string())
            .fetch_all(&self.pool)
            .await?;

        let mut results = Vec::new();
        for r in rows {
            let dec_token = self.encryptor.decrypt(r.try_get("primary_token")?)?;
            let rfr_opt: Option<String> = r.try_get("refresh_token")?;
            let dec_refresh = if let Some(enc) = rfr_opt {
                Some(self.encryptor.decrypt(&enc)?)
            } else {
                None
            };
            let data_opt: Option<String> = r.try_get("additional_data")?;
            let dec_data = if let Some(x) = data_opt {
                let js = self.encryptor.decrypt(&x)?;
                Some(serde_json::from_str(&js)?)
            } else {
                None
            };

            results.push(PlatformCredential {
                credential_id: r.try_get("credential_id")?,
                platform: r.try_get::<String, _>("platform")?.parse()?,
                platform_id: r.try_get("platform_id")?,
                credential_type: r.try_get::<String, _>("credential_type")?.parse()?,
                user_id: r.try_get("user_id")?,
                user_name: r.try_get("user_name")?,
                primary_token: dec_token,
                refresh_token: dec_refresh,
                additional_data: dec_data,
                expires_at: r.try_get("expires_at")?,
                created_at: r.try_get("created_at")?,
                updated_at: r.try_get("updated_at")?,
                is_bot: r.try_get("is_bot")?,
            });
        }
        Ok(results)
    }
}
