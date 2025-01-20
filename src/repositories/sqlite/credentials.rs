//! src/repositories/sqlite/credentials.rs
use std::str::FromStr;
use sqlx::{Pool, Sqlite};
use crate::Error;
use crate::crypto::Encryptor;
use crate::repositories::CredentialsRepository;
use crate::models::{Platform, PlatformCredential};
use chrono::{Utc, Duration};

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

        sqlx::query!(
            r#"INSERT INTO platform_credentials
               (credential_id, platform, credential_type, user_id, primary_token,
                refresh_token, additional_data, expires_at, created_at, updated_at)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
               ON CONFLICT (platform, user_id) DO UPDATE SET
                   primary_token = excluded.primary_token,
                   refresh_token = excluded.refresh_token,
                   additional_data = excluded.additional_data,
                   expires_at = excluded.expires_at,
                   updated_at = excluded.updated_at
            "#,
            creds.credential_id,
            platform_str,
            cred_type_str,
            creds.user_id,
            encrypted_token,
            encrypted_refresh,
            encrypted_data,
            creds.expires_at,
            creds.created_at,
            creds.updated_at
        )
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

        let record = sqlx::query!(
            r#"SELECT *
               FROM platform_credentials
               WHERE platform = ? AND user_id = ?
            "#,
            platform_str,
            user_id
        )
            .fetch_optional(&self.pool)
            .await?;

        match record {
            Some(r) => {
                let decrypted_token = self.encryptor.decrypt(&r.primary_token)?;
                let decrypted_refresh = if let Some(ref_token) = r.refresh_token {
                    Some(self.encryptor.decrypt(&ref_token)?)
                } else {
                    None
                };
                let decrypted_data = if let Some(data_str) = r.additional_data {
                    let json_str = self.encryptor.decrypt(&data_str)?;
                    Some(serde_json::from_str(&json_str)?)
                } else {
                    None
                };

                Ok(Some(PlatformCredential {
                    credential_id: r.credential_id,
                    platform: platform.clone(),
                    credential_type: r.credential_type.parse()?,
                    user_id: r.user_id,
                    primary_token: decrypted_token,
                    refresh_token: decrypted_refresh,
                    additional_data: decrypted_data,
                    expires_at: r.expires_at,
                    created_at: r.created_at,
                    updated_at: r.updated_at,
                }))
            }
            None => Ok(None),
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

        sqlx::query!(
            r#"UPDATE platform_credentials
               SET primary_token = ?,
                   refresh_token = ?,
                   additional_data = ?,
                   expires_at = ?,
                   updated_at = ?
               WHERE platform = ? AND user_id = ?"#,
            encrypted_token,
            encrypted_refresh,
            encrypted_data,
            creds.expires_at,
            creds.updated_at,
            platform_str,
            creds.user_id
        )
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

        sqlx::query!(
            r#"DELETE FROM platform_credentials
               WHERE platform = ? AND user_id = ?
            "#,
            platform_str,
            user_id
        )
            .execute(&self.pool)
            .await?;

        Ok(())
    }
}

impl SqliteCredentialsRepository {
    /// Returns all credentials that have an `expires_at` within the specified duration
    /// from "now". For example, you can pass `Duration::minutes(10)` to get all tokens
    /// expiring in the next 10 minutes.
    pub async fn get_expiring_credentials(
        &self,
        within: Duration,
    ) -> Result<Vec<PlatformCredential>, Error> {
        let now = Utc::now().naive_utc();
        let cutoff = now + within;

        // Retrieve any rows where expires_at <= cutoff
        let rows = sqlx::query!(
            r#"SELECT *
               FROM platform_credentials
               WHERE expires_at IS NOT NULL
                 AND expires_at <= ?
            "#,
            cutoff
        )
            .fetch_all(&self.pool)
            .await?;

        let mut results = Vec::new();
        for row in rows {
            let platform = Platform::from_str(&row.platform)
                .map_err(|e| Error::Platform(e.to_string()))?;

            let decrypted_token = self.encryptor.decrypt(&row.primary_token)?;
            let decrypted_refresh = if let Some(ref_token) = row.refresh_token {
                Some(self.encryptor.decrypt(&ref_token)?)
            } else {
                None
            };
            let decrypted_data = if let Some(data_str) = row.additional_data {
                let json_str = self.encryptor.decrypt(&data_str)?;
                Some(serde_json::from_str(&json_str)?)
            } else {
                None
            };

            results.push(PlatformCredential {
                credential_id: row.credential_id,
                platform,
                credential_type: row.credential_type.parse()?,
                user_id: row.user_id,
                primary_token: decrypted_token,
                refresh_token: decrypted_refresh,
                additional_data: decrypted_data,
                expires_at: row.expires_at,
                created_at: row.created_at,
                updated_at: row.updated_at,
            });
        }

        Ok(results)
    }
}
