// =============================================================================
// maowbot-core/src/auth/manager.rs
//   - Removed “label” usage. We now store only one config row per platform.
// =============================================================================

use std::collections::HashMap;
use std::sync::Arc;

use crate::auth::{PlatformAuthenticator, AuthenticationPrompt, AuthenticationResponse};
use crate::Error;
use crate::models::{Platform, PlatformCredential};
use crate::repositories::{BotConfigRepository, CredentialsRepository};
use crate::repositories::postgres::platform_config::PlatformConfigRepository;

use crate::platforms::discord::auth::DiscordAuthenticator;
use crate::platforms::twitch_helix::auth::TwitchAuthenticator;
use crate::platforms::vrchat::auth::VRChatAuthenticator;
use crate::platforms::twitch_irc::auth::TwitchIrcAuthenticator;

/// AuthManager: manages platform authenticators, reading config from the DB.
pub struct AuthManager {
    pub credentials_repo: Box<dyn CredentialsRepository + Send + Sync>,
    pub platform_config_repo: Arc<dyn PlatformConfigRepository + Send + Sync>,
    pub bot_config_repo: Arc<dyn BotConfigRepository + Send + Sync>,
    pub authenticators: HashMap<Platform, Box<dyn PlatformAuthenticator + Send + Sync>>,
}

impl AuthManager {
    pub fn new(
        credentials_repo: Box<dyn CredentialsRepository + Send + Sync>,
        platform_config_repo: Arc<dyn PlatformConfigRepository + Send + Sync>,
        bot_config_repo: Arc<dyn BotConfigRepository + Send + Sync>,
    ) -> Self {
        Self {
            credentials_repo,
            platform_config_repo,
            bot_config_repo,
            authenticators: HashMap::new(),
        }
    }

    pub fn register_authenticator(
        &mut self,
        platform: Platform,
        authenticator: Box<dyn PlatformAuthenticator + Send + Sync + 'static>,
    ) {
        self.authenticators.insert(platform, authenticator);
    }

    /// This older convenience method is left for any backward usage.
    pub async fn authenticate_platform(&mut self, platform: Platform) -> Result<PlatformCredential, Error> {
        self.authenticate_platform_for_role(platform, false).await
    }

    pub async fn authenticate_platform_for_role(
        &mut self,
        platform: Platform,
        is_bot: bool,
    ) -> Result<PlatformCredential, Error> {
        let _ = self.begin_auth_flow(platform.clone(), is_bot).await?;
        Err(Error::Auth(
            "This function expects a code, but none was provided (use the 2-step flow)".into(),
        ))
    }

    /// Start an auth flow for the given platform. Returns the “redirect” or user prompt URL.
    pub async fn begin_auth_flow(
        &mut self,
        platform: Platform,
        is_bot: bool,
    ) -> Result<String, Error> {
        let platform_str = match &platform {
            Platform::Twitch => "twitch",
            Platform::Discord => "discord",
            Platform::VRChat => "vrchat",
            Platform::TwitchIRC => "twitch-irc",
        };

        // Single config row for this platform:
        let maybe_conf = self.platform_config_repo.get_by_platform(platform_str).await?;
        let (client_id, client_secret) = if let Some(conf_row) = maybe_conf {
            let cid = conf_row.client_id.unwrap_or_default();
            let csec = conf_row.client_secret;
            (cid, csec)
        } else {
            return Err(Error::Auth(format!(
                "No platform_config found for platform={}",
                platform_str
            )));
        };

        // Build authenticator
        let authenticator: Box<dyn PlatformAuthenticator + Send + Sync> = match platform {
            Platform::Discord => {
                Box::new(DiscordAuthenticator::new(
                    Some(client_id),
                    client_secret,
                ))
            }
            Platform::Twitch => {
                Box::new(TwitchAuthenticator::new(
                    client_id,
                    client_secret,
                ))
            }
            Platform::VRChat => {
                Box::new(VRChatAuthenticator::new())
            }
            Platform::TwitchIRC => {
                Box::new(TwitchIrcAuthenticator::new())
            }
        };
        self.authenticators.insert(platform.clone(), authenticator);

        // set is_bot
        if let Some(auth) = self.authenticators.get_mut(&platform) {
            auth.set_is_bot(is_bot);
            auth.initialize().await?;
            if let Ok(prompt) = auth.start_authentication().await {
                match prompt {
                    AuthenticationPrompt::Browser { url } => {
                        return Ok(url);
                    }
                    AuthenticationPrompt::Code { message } => {
                        return Err(Error::Auth(message));
                    }
                    AuthenticationPrompt::ApiKey { message } => {
                        return Ok(format!("(API key) {}", message));
                    }
                    AuthenticationPrompt::MultipleKeys { .. } => {
                        return Ok("(Multiple keys required) handle in TUI".into());
                    }
                    AuthenticationPrompt::TwoFactor { message } => {
                        return Ok(format!("(2FA) {}", message));
                    }
                    AuthenticationPrompt::None => {
                        return Ok("(No prompt needed)".into());
                    }
                }
            }
        }
        Err(Error::Platform(format!(
            "Could not begin auth flow for platform={:?}",
            platform
        )))
    }

    pub async fn complete_auth_flow(
        &mut self,
        platform: Platform,
        code: String,
    ) -> Result<PlatformCredential, Error> {
        let authenticator = self
            .authenticators
            .get_mut(&platform)
            .ok_or_else(|| Error::Platform(format!("No authenticator for {platform:?}")))?;

        let cred = authenticator
            .complete_authentication(AuthenticationResponse::Code(code))
            .await?;
        self.credentials_repo.store_credentials(&cred).await?;
        Ok(cred)
    }

    pub async fn store_credentials(&self, cred: &PlatformCredential) -> Result<(), Error> {
        self.credentials_repo.store_credentials(cred).await
    }

    pub async fn get_credentials(
        &self,
        platform: &Platform,
        user_id: &str,
    ) -> Result<Option<PlatformCredential>, Error> {
        self.credentials_repo.get_credentials(platform, user_id).await
    }

    pub async fn revoke_credentials(
        &mut self,
        platform: &Platform,
        user_id: &str,
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
        user_id: &str,
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

    /// Replaces the old 'insert_platform_config' that needed a label. We just upsert by platform.
    pub async fn create_platform_config(
        &self,
        platform_str: &str,
        client_id: String,
        client_secret: Option<String>,
    ) -> Result<(), Error> {
        self.platform_config_repo
            .upsert_platform_config(platform_str, Some(client_id), client_secret)
            .await?;
        Ok(())
    }

    pub async fn count_platform_configs_for(&self, platform_str: &str) -> Result<usize, Error> {
        let n = self.platform_config_repo.count_for_platform(platform_str).await?;
        Ok(n as usize)
    }
}