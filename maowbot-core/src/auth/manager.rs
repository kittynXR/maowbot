// File: maowbot-core/src/auth/manager.rs

use std::collections::HashMap;
use crate::auth::{AuthenticationHandler, PlatformAuthenticator};
use crate::Error;
use crate::models::{Platform, PlatformCredential};
use crate::repositories::CredentialsRepository;

pub struct AuthManager {
    credentials_repo: Box<dyn CredentialsRepository + Send + Sync + 'static>,
    /// Keyed by `Platform`, e.g. Twitch -> TwitchAuthenticator
    authenticators: HashMap<Platform, Box<dyn PlatformAuthenticator + Send + Sync + 'static>>,
    auth_handler: Box<dyn AuthenticationHandler + Send + Sync + 'static>,
}

impl AuthManager {
    pub fn new(
        credentials_repo: Box<dyn CredentialsRepository + Send + Sync + 'static>,
        auth_handler: Box<dyn AuthenticationHandler + Send + Sync + 'static>,
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
        authenticator: Box<dyn PlatformAuthenticator + Send + Sync + 'static>,
    ) {
        self.authenticators.insert(platform, authenticator);
    }

    /// Original method for authentication with no explicit `is_bot` flag.
    /// For backward-compat or for platforms that don’t need `is_bot`.
    pub async fn authenticate_platform(
        &mut self,
        platform: Platform
    ) -> Result<PlatformCredential, Error> {
        self.inner_authenticate(platform, None).await
    }

    /// New method that sets an `is_bot` flag in the final credential.
    pub async fn authenticate_platform_for_role(
        &mut self,
        platform: Platform,
        is_bot: bool,
    ) -> Result<PlatformCredential, Error> {
        self.inner_authenticate(platform, Some(is_bot)).await
    }

    async fn inner_authenticate(
        &mut self,
        platform: Platform,
        is_bot_override: Option<bool>,
    ) -> Result<PlatformCredential, Error> {
        let authenticator = self.authenticators.get_mut(&platform)
            .ok_or_else(|| Error::Platform(format!("No authenticator for {:?}", platform)))?;

        // Provide a small hook if the authenticator wants to track is_bot
        if let Some(b) = is_bot_override {
            // We do a downcast if the authenticator supports a set_is_bot method
            // or use any specialized approach. For general trait objects, you might
            // store a flag in the authenticator itself. This example uses a new method:
            authenticator.set_is_bot(b);
        }

        authenticator.initialize().await?;

        let mut prompt = authenticator.start_authentication().await?;
        loop {
            let response = self.auth_handler.handle_prompt(prompt.clone()).await?;
            match authenticator.complete_authentication(response).await {
                Ok(mut credential) => {
                    // If an override was given, ensure the final credential has is_bot set
                    if let Some(b) = is_bot_override {
                        credential.is_bot = b;
                    }
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