//! src/repositories/sqlite/credentials.rs
use std::str::FromStr;
use sqlx::{Pool, Sqlite, Row};
use crate::Error;
use crate::crypto::Encryptor;
use crate::repositories::CredentialsRepository;
use crate::models::{Platform, PlatformCredential};
use chrono::{Utc, Duration};
use chrono::NaiveDateTime;
use crate::utils::time::{to_epoch, from_epoch};

#[derive(Clone)]
pub struct SqliteCredentialsRepository {
    pool: Pool<Sqlite>,
    encryptor: Encryptor,
}

impl SqliteCredentialsRepository {
    pub fn new(pool: Pool<Sqlite>, encryptor: Encryptor) -> Self {
        Self { pool, encryptor }
    }
}

#[async_trait::async_trait]
impl CredentialsRepository for SqliteCredentialsRepository {
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

        sqlx::query(
            r#"
            INSERT INTO platform_credentials
               (credential_id, platform, credential_type, user_id, primary_token,
                refresh_token, additional_data, expires_at, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT (platform, user_id) DO UPDATE SET
                primary_token = excluded.primary_token,
                refresh_token = excluded.refresh_token,
                additional_data = excluded.additional_data,
                expires_at = excluded.expires_at,
                updated_at = excluded.updated_at
            "#
        )
            .bind(&creds.credential_id)
            .bind(&platform_str)
            .bind(&cred_type_str)
            .bind(&creds.user_id)
            .bind(encrypted_token)
            .bind(encrypted_refresh)
            .bind(encrypted_data)
            // If expires_at is an Option<NaiveDateTime>, convert it if present.
            .bind(creds.expires_at.map(|dt| to_epoch(dt)))
            .bind(to_epoch(creds.created_at))
            .bind(to_epoch(creds.updated_at))
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    async fn get_credentials(
        &self,
        platform: &Platform,
        user_id: &str
    ) -> Result<Option<PlatformCredential>, Error> {
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
            WHERE platform = ? AND user_id = ?
            "#
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

            let expires_at: Option<NaiveDateTime> = {
                let epoch_opt: Option<i64> = r.try_get("expires_at")?;
                epoch_opt.map(|e| from_epoch(e))
            };
            let created_at: NaiveDateTime = from_epoch(r.try_get::<i64, _>("created_at")?);
            let updated_at: NaiveDateTime = from_epoch(r.try_get::<i64, _>("updated_at")?);

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
            Some(token) => Some(self.encryptor.encrypt(token)?),
            None => None,
        };
        let encrypted_data = match &creds.additional_data {
            Some(data) => Some(self.encryptor.encrypt(&data.to_string())?),
            None => None,
        };

        sqlx::query(
            r#"
            UPDATE platform_credentials
               SET primary_token = ?,
                   refresh_token = ?,
                   additional_data = ?,
                   expires_at = ?,
                   updated_at = ?
            WHERE platform = ? AND user_id = ?
            "#
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

    async fn delete_credentials(
        &self,
        platform: &Platform,
        user_id: &str
    ) -> Result<(), Error> {
        let platform_str = platform.to_string();

        sqlx::query(
            r#"
            DELETE FROM platform_credentials
            WHERE platform = ? AND user_id = ?
            "#
        )
            .bind(platform_str)
            .bind(user_id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }
}

impl SqliteCredentialsRepository {
    /// Returns all credentials that have an `expires_at` within the specified duration
    /// from "now". For example, `Duration::minutes(10)` => all tokens expiring in next 10 min.
    pub async fn get_expiring_credentials(
        &self,
        within: Duration,
    ) -> Result<Vec<PlatformCredential>, Error> {
        let now = Utc::now().naive_utc();
        let cutoff = now + within;

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
              AND expires_at <= ?
            "#
        )
            .bind(cutoff)
            .fetch_all(&self.pool)
            .await?;

        let mut results = Vec::new();
        for r in rows {
            let platform_str: String = r.try_get("platform")?;
            let decrypted_token = self.encryptor.decrypt(r.try_get("primary_token")?)?;
            let ref_token_opt: Option<String> = r.try_get("refresh_token")?;
            let decrypted_refresh = if let Some(s) = ref_token_opt {
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

            results.push(PlatformCredential {
                credential_id: r.try_get("credential_id")?,
                platform: Platform::from_str(&platform_str)
                    .map_err(|e| Error::Platform(e.to_string()))?,
                credential_type: r.try_get::<String, _>("credential_type")?.parse()?,
                user_id: r.try_get("user_id")?,
                primary_token: decrypted_token,
                refresh_token: decrypted_refresh,
                additional_data: decrypted_data,
                expires_at: r.try_get("expires_at")?,
                created_at: r.try_get("created_at")?,
                updated_at: r.try_get("updated_at")?,
            });
        }

        Ok(results)
    }
}
