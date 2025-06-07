use crate::Error;
use async_trait::async_trait;
use maowbot_common::models::platform::Platform;
use maowbot_common::traits::platform_traits::PlatformAuth;
use sqlx::{Pool, Postgres};
use tracing::info;

/// OBS uses simple password authentication, not OAuth
pub struct ObsAuthenticator {
    instance_number: u32,
    pool: Pool<Postgres>,
}

impl ObsAuthenticator {
    pub fn new(instance_number: u32, pool: Pool<Postgres>) -> Self {
        Self {
            instance_number,
            pool,
        }
    }
}

#[async_trait]
impl PlatformAuth for ObsAuthenticator {
    async fn authenticate(&mut self) -> Result<(), Error> {
        // For OBS, authentication is handled during connection with password
        // No OAuth flow needed
        Ok(())
    }

    async fn refresh_auth(&mut self) -> Result<(), Error> {
        // OBS doesn't have refresh tokens
        Ok(())
    }

    async fn revoke_auth(&mut self) -> Result<(), Error> {
        // No token revocation for OBS
        Ok(())
    }

    async fn is_authenticated(&self) -> Result<bool, Error> {
        // Check if this instance has connection info in the database
        let result = sqlx::query(
            r#"
            SELECT instance_number 
            FROM obs_instances 
            WHERE instance_number = $1
            "#
        )
        .bind(self.instance_number as i32)
        .fetch_optional(&self.pool)
        .await?;
        
        Ok(result.is_some())
    }
}