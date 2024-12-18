use std::str::FromStr;
// src/repositories/sqlite/credentials.rs
use super::*;
use crate::crypto::Encryptor;
use sqlx::{Pool, Sqlite};
use crate::repositories::CredentialsRepository;
use crate::models::{CredentialType, Platform, PlatformCredential};

pub struct SqliteCredentialsRepository {
    pool: Pool<Sqlite>,
    encryptor: Encryptor,
}

impl SqliteCredentialsRepository {
    pub fn new(pool: Pool<Sqlite>, encryptor: Encryptor) -> Self {
        Self { pool, encryptor }
    }
}

#[async_trait]
impl CredentialsRepository for SqliteCredentialsRepository {
    async fn store_credentials(&self, creds: &PlatformCredential) -> Result<(), Error> {
        let platform_str = creds.platform.to_string();
        let cred_type_str = creds.credential_type.to_string(); // Store in a variable
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
            updated_at = excluded.updated_at"#,
            creds.credential_id,
            platform_str,
            cred_type_str, // Use the stored string
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
            r#"SELECT * FROM platform_credentials
            WHERE platform = ? AND user_id = ?"#,
            platform_str,
            user_id
        )
            .fetch_optional(&self.pool)
            .await?;

        match record {
            Some(r) => {
                let decrypted_token = self.encryptor.decrypt(&r.primary_token)?.to_string();

                let decrypted_refresh = if let Some(token_str) = &r.refresh_token {
                    Some(self.encryptor.decrypt(token_str)?.to_string())
                } else {
                    None
                };

                let decrypted_data = if let Some(data_str) = &r.additional_data {
                    let decrypted = self.encryptor.decrypt(data_str)?.to_string();
                    Some(serde_json::from_str(&decrypted)?)
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
            None => Ok(None)
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
            WHERE platform = ? AND user_id = ?"#,
            platform_str,
            user_id
        )
            .execute(&self.pool)
            .await?;

        Ok(())
    }
}