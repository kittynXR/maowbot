// src/repositories/postgres/credentials.rs
use crate::Error;
use crate::crypto::Encryptor;
use crate::models::{Platform, PlatformCredential};
use crate::utils::time::{to_epoch, from_epoch};
use async_trait::async_trait;
use chrono::Duration;
use sqlx::{Pool, Postgres, Row};
use std::str::FromStr;

#[async_trait]
pub trait CredentialsRepository: Send + Sync {
    async fn store_credentials(&self, creds: &PlatformCredential) -> Result<(), Error>;
    async fn get_credentials(&self, platform: &Platform, user_id: &str) -> Result<Option<PlatformCredential>, Error>;
    async fn update_credentials(&self, creds: &PlatformCredential) -> Result<(), Error>;
    async fn delete_credentials(&self, platform: &Platform, user_id: &str) -> Result<(), Error>;

    /// Additional helper for fetching any credentials expiring within a certain duration.
    async fn get_expiring_credentials(&self, within: Duration) -> Result<Vec<PlatformCredential>, Error>;
}

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
        let encrypted_token = self.encryptor.encrypt(&creds.primary_token)?;
        let encrypted_refresh = match &creds.refresh_token {
            Some(token) => Some(self.encryptor.encrypt(token)?),
            None => None,
        };
        let encrypted_data = match &creds.additional_data {
            Some(data) => Some(self.encryptor.encrypt(&data.to_string())?),
            None => None,
        };

        // Upsert approach with Postgres can be done with ON CONFLICT (platform, user_id).
        // Here is an example using a single statement:
        sqlx::query(
            r#"
            INSERT INTO platform_credentials
               (credential_id, platform, credential_type, user_id, primary_token,
                refresh_token, additional_data, expires_at, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            ON CONFLICT (platform, user_id) DO UPDATE
               SET primary_token = EXCLUDED.primary_token,
                   refresh_token = EXCLUDED.refresh_token,
                   additional_data = EXCLUDED.additional_data,
                   expires_at = EXCLUDED.expires_at,
                   updated_at = EXCLUDED.updated_at
            "#,
        )
            .bind(&creds.credential_id)
            .bind(&platform_str)
            .bind(&cred_type_str)
            .bind(&creds.user_id)
            .bind(encrypted_token)
            .bind(encrypted_refresh)
            .bind(encrypted_data)
            .bind(creds.expires_at.map(|dt| to_epoch(dt)))
            .bind(to_epoch(creds.created_at))
            .bind(to_epoch(creds.updated_at))
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    async fn get_credentials(&self, platform: &Platform, user_id: &str) -> Result<Option<PlatformCredential>, Error> {
        let platform_str = platform.to_string();
        let row = sqlx::query(
            r#"
            SELECT
                credential_id,
                platform,
                credential_type,
                user_id,
                primary_token,
                refresh_token,
                additional_data,
                expires_at,
                created_at,
                updated_at
            FROM platform_credentials
            WHERE platform = $1 AND user_id = $2
            "#,
        )
            .bind(&platform_str)
            .bind(user_id)
            .fetch_optional(&self.pool)
            .await?;

        if let Some(r) = row {
            let decrypted_token = self.encryptor.decrypt(r.try_get("primary_token")?)?;
            let refresh_opt: Option<String> = r.try_get("refresh_token")?;
            let decrypted_refresh = if let Some(ref_token) = refresh_opt {
                Some(self.encryptor.decrypt(&ref_token)?)
            } else {
                None
            };
            let data_opt: Option<String> = r.try_get("additional_data")?;
            let decrypted_data = if let Some(data_str) = data_opt {
                let json_str = self.encryptor.decrypt(&data_str)?;
                Some(serde_json::from_str(&json_str)?)
            } else {
                None
            };

            let expires_epoch: Option<i64> = r.try_get("expires_at")?;
            let expires_at = expires_epoch.map(from_epoch);

            let created_at = from_epoch(r.try_get::<i64, _>("created_at")?);
            let updated_at = from_epoch(r.try_get::<i64, _>("updated_at")?);

            Ok(Some(PlatformCredential {
                credential_id: r.try_get("credential_id")?,
                platform: Platform::from_str(&r.try_get::<String, _>("platform")?)
                    .map_err(|e| Error::Platform(e.to_string()))?,
                credential_type: r.try_get::<String, _>("credential_type")?.parse()?,
                user_id: r.try_get("user_id")?,
                primary_token: decrypted_token,
                refresh_token: decrypted_refresh,
                additional_data: decrypted_data,
                expires_at,
                created_at,
                updated_at,
            }))
        } else {
            Ok(None)
        }
    }

    async fn update_credentials(&self, creds: &PlatformCredential) -> Result<(), Error> {
        let platform_str = creds.platform.to_string();
        let encrypted_token = self.encryptor.encrypt(&creds.primary_token)?;
        let encrypted_refresh = match &creds.refresh_token {
            Some(t) => Some(self.encryptor.encrypt(t)?),
            None => None,
        };
        let encrypted_data = match &creds.additional_data {
            Some(d) => Some(self.encryptor.encrypt(&d.to_string())?),
            None => None,
        };

        sqlx::query(
            r#"
            UPDATE platform_credentials
            SET primary_token = $1,
                refresh_token = $2,
                additional_data = $3,
                expires_at = $4,
                updated_at = $5
            WHERE platform = $6 AND user_id = $7
            "#,
        )
            .bind(encrypted_token)
            .bind(encrypted_refresh)
            .bind(encrypted_data)
            .bind(creds.expires_at.map(|dt| to_epoch(dt)))
            .bind(to_epoch(creds.updated_at))
            .bind(platform_str)
            .bind(&creds.user_id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    async fn delete_credentials(&self, platform: &Platform, user_id: &str) -> Result<(), Error> {
        let platform_str = platform.to_string();
        sqlx::query(
            r#"
            DELETE FROM platform_credentials
            WHERE platform = $1 AND user_id = $2
            "#,
        )
            .bind(&platform_str)
            .bind(user_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn get_expiring_credentials(&self, within: Duration) -> Result<Vec<PlatformCredential>, Error> {
        let now = chrono::Utc::now().naive_utc();
        let cutoff = now + within;
        let cutoff_epoch = to_epoch(cutoff);

        let rows = sqlx::query(
            r#"
            SELECT
                credential_id,
                platform,
                credential_type,
                user_id,
                primary_token,
                refresh_token,
                additional_data,
                expires_at,
                created_at,
                updated_at
            FROM platform_credentials
            WHERE expires_at IS NOT NULL
              AND expires_at <= $1
            "#,
        )
            .bind(cutoff_epoch)
            .fetch_all(&self.pool)
            .await?;

        let mut results = Vec::new();
        for r in rows {
            let decrypted_token = self.encryptor.decrypt(r.try_get("primary_token")?)?;
            let ref_opt: Option<String> = r.try_get("refresh_token")?;
            let decrypted_refresh = if let Some(s) = ref_opt {
                Some(self.encryptor.decrypt(&s)?)
            } else {
                None
            };
            let data_opt: Option<String> = r.try_get("additional_data")?;
            let decrypted_data = if let Some(data_str) = data_opt {
                let json_str = self.encryptor.decrypt(&data_str)?;
                Some(serde_json::from_str(&json_str)?)
            } else {
                None
            };

            let expires_epoch: Option<i64> = r.try_get("expires_at")?;
            let expires_at = expires_epoch.map(from_epoch);

            let created_at = from_epoch(r.try_get::<i64, _>("created_at")?);
            let updated_at = from_epoch(r.try_get::<i64, _>("updated_at")?);

            let platform_str: String = r.try_get("platform")?;
            results.push(PlatformCredential {
                credential_id: r.try_get("credential_id")?,
                platform: Platform::from_str(&platform_str)
                    .map_err(|e| Error::Platform(e.to_string()))?,
                credential_type: r.try_get::<String, _>("credential_type")?.parse()?,
                user_id: r.try_get("user_id")?,
                primary_token: decrypted_token,
                refresh_token: decrypted_refresh,
                additional_data: decrypted_data,
                expires_at,
                created_at,
                updated_at,
            });
        }

        Ok(results)
    }
}