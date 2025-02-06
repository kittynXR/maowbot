// File: maowbot-core/src/auth/manager.rs

use std::collections::HashMap;
use crate::auth::{PlatformAuthenticator, AuthenticationPrompt, AuthenticationResponse};
use crate::Error;
use crate::models::{Platform, PlatformCredential};
use crate::repositories::CredentialsRepository;

/// AuthManager is responsible for coordinating the authentication flows for different platforms
/// and storing the resulting credentials.
pub struct AuthManager {
    pub credentials_repo: Box<dyn CredentialsRepository + Send + Sync + 'static>,
    pub authenticators: HashMap<Platform, Box<dyn PlatformAuthenticator + Send + Sync + 'static>>,
}

impl AuthManager {
    pub fn new(
        credentials_repo: Box<dyn CredentialsRepository + Send + Sync + 'static>
    ) -> Self {
        Self {
            credentials_repo,
            authenticators: HashMap::new(),
        }
    }

    /// Register an authenticator for a given platform.
    pub fn register_authenticator(
        &mut self,
        platform: Platform,
        authenticator: Box<dyn PlatformAuthenticator + Send + Sync + 'static>,
    ) {
        self.authenticators.insert(platform, authenticator);
    }

    /// A convenience method for platforms that do not require an is_bot flag.
    pub async fn authenticate_platform(
        &mut self,
        platform: Platform
    ) -> Result<PlatformCredential, Error> {
        self.authenticate_platform_for_role(platform, false).await
    }

    /// This method is kept for backward compatibility.
    /// It calls the interactive two‑step flow and then returns an error if no code is provided.
    pub async fn authenticate_platform_for_role(
        &mut self,
        platform: Platform,
        is_bot: bool,
    ) -> Result<PlatformCredential, Error> {
        let _ = self.begin_auth_flow(platform.clone(), is_bot).await?;
        Err(Error::Auth("This function expects a code, but none was provided (use begin_auth_flow and complete_auth_flow).".into()))
    }

    /// Step 1 of the interactive flow: initialize the authenticator and obtain an authentication prompt.
    pub async fn begin_auth_flow(
        &mut self,
        platform: Platform,
        is_bot: bool,
    ) -> Result<AuthenticationPrompt, Error> {
        let authenticator = self.authenticators.get_mut(&platform)
            .ok_or_else(|| Error::Platform(format!("No authenticator for {:?}", platform)))?;
        authenticator.set_is_bot(is_bot);
        authenticator.initialize().await?;
        let prompt = authenticator.start_authentication().await?;
        Ok(prompt)
    }

    /// Step 2 of the interactive flow: supply the code (e.g. from a local callback server) to complete the authentication.
    pub async fn complete_auth_flow(
        &mut self,
        platform: Platform,
        code: String,
    ) -> Result<PlatformCredential, Error> {
        let authenticator = self.authenticators.get_mut(&platform)
            .ok_or_else(|| Error::Platform(format!("No authenticator for {:?}", platform)))?;
        let cred = authenticator
            .complete_authentication(AuthenticationResponse::Code(code))
            .await?;
        self.credentials_repo.store_credentials(&cred).await?;
        Ok(cred)
    }

    /// Store the given credential in the repository.
    pub async fn store_credentials(&self, cred: &PlatformCredential) -> Result<(), Error> {
        self.credentials_repo.store_credentials(cred).await
    }

    /// Retrieve credentials by platform and user ID.
    pub async fn get_credentials(
        &self,
        platform: &Platform,
        user_id: &str
    ) -> Result<Option<PlatformCredential>, Error> {
        self.credentials_repo.get_credentials(platform, user_id).await
    }

    /// Revoke the credentials for a given platform and user ID.
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

    /// Refresh credentials for a given platform and user ID.
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

    /// Validate an existing credential.
    pub async fn validate_credentials(
        &mut self,
        cred: &PlatformCredential
    ) -> Result<bool, Error> {
        let authenticator = self.authenticators.get_mut(&cred.platform)
            .ok_or_else(|| Error::Platform(format!("No authenticator for {:?}", cred.platform)))?;
        authenticator.validate(cred).await
    }
}