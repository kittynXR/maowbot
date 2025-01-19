// File: src/auth/manager.rs

use std::collections::HashMap;
use crate::auth::{AuthenticationHandler, PlatformAuthenticator};
use crate::Error;
use crate::models::{Platform, PlatformCredential};
use crate::repositories::CredentialsRepository;

pub struct AuthManager {
    credentials_repo: Box<dyn CredentialsRepository>,
    authenticators: HashMap<Platform, Box<dyn PlatformAuthenticator>>,
    auth_handler: Box<dyn AuthenticationHandler>,
}

impl AuthManager {
    pub fn new(
        credentials_repo: Box<dyn CredentialsRepository>,
        auth_handler: Box<dyn AuthenticationHandler>,
    ) -> Self {
        Self {
            credentials_repo,
            authenticators: HashMap::new(),
            auth_handler,
        }
    }

    pub fn register_authenticator(
        &mut self,
        platform: Platform,
        authenticator: Box<dyn PlatformAuthenticator>
    ) {
        self.authenticators.insert(platform, authenticator);
    }

    pub async fn authenticate_platform(
        &mut self,
        platform: Platform
    ) -> Result<PlatformCredential, Error> {
        let authenticator = self.authenticators.get_mut(&platform)
            .ok_or_else(|| Error::Platform(format!("No authenticator for {:?}", platform)))?;

        authenticator.initialize().await?;

        let mut prompt = authenticator.start_authentication().await?;
        loop {
            let response = self.auth_handler.handle_prompt(prompt.clone()).await?;
            match authenticator.complete_authentication(response).await {
                Ok(credential) => {
                    self.credentials_repo.store_credentials(&credential).await?;
                    return Ok(credential);
                }
                Err(Error::Auth(msg)) if msg == "2FA required" => {
                    prompt = authenticator.start_authentication().await?;
                    continue;
                }
                Err(e) => return Err(e),
            }
        }
    }

    pub async fn store_credentials(&self, cred: &PlatformCredential) -> Result<(), Error> {
        self.credentials_repo.store_credentials(cred).await
    }

    pub async fn get_credentials(
        &self,
        platform: &Platform,
        user_id: &str
    ) -> Result<Option<PlatformCredential>, Error> {
        self.credentials_repo.get_credentials(platform, user_id).await
    }

    pub async fn revoke_credentials(
        &mut self,
        platform: &Platform,
        user_id: &str
    ) -> Result<(), Error> {
        if let Some(cred) = self.credentials_repo.get_credentials(platform, user_id).await? {
            let authenticator = self.authenticators.get_mut(platform)
                .ok_or_else(|| Error::Platform(format!("No authenticator for {:?}", platform)))?;

            authenticator.revoke(&cred).await?;
            self.credentials_repo.delete_credentials(platform, user_id).await?;
        }
        Ok(())
    }

    pub async fn refresh_platform_credentials(
        &mut self,
        platform: &Platform,
        user_id: &str
    ) -> Result<PlatformCredential, Error> {
        let cred = self.credentials_repo
            .get_credentials(platform, user_id).await?
            .ok_or_else(|| Error::Auth("No credentials found".into()))?;

        let authenticator = self.authenticators.get_mut(platform)
            .ok_or_else(|| Error::Platform(format!("No authenticator for {:?}", platform)))?;

        let refreshed = authenticator.refresh(&cred).await?;
        self.credentials_repo.store_credentials(&refreshed).await?;

        Ok(refreshed)
    }

    pub async fn validate_credentials(
        &mut self,
        cred: &PlatformCredential
    ) -> Result<bool, Error> {
        let authenticator = self.authenticators.get_mut(&cred.platform)
            .ok_or_else(|| Error::Platform(format!("No authenticator for {:?}", cred.platform)))?;

        authenticator.validate(cred).await
    }
}
