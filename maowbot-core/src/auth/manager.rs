// maowbot-core/src/auth/manager.rs
//
// This updated file changes the old `begin_auth_flow` to accept a label,
// and adds a new helper `create_auth_config` so the TUI can insert a row
// if none exists. It also adds a `count_auth_configs_for_platform` method
// so the TUI can propose labels based on how many are already there.

use std::collections::HashMap;
use std::sync::Arc;

use crate::auth::{PlatformAuthenticator, AuthenticationPrompt, AuthenticationResponse};
use crate::Error;
use crate::models::{Platform, PlatformCredential};
use crate::repositories::{BotConfigRepository, CredentialsRepository};
use crate::repositories::postgres::auth_config::AuthConfigRepository;

use crate::platforms::discord::auth::DiscordAuthenticator;
use crate::platforms::twitch_helix::auth::TwitchAuthenticator;
use crate::platforms::vrchat::auth::VRChatAuthenticator;
use crate::platforms::twitch_irc::auth::TwitchIrcAuthenticator;

/// AuthManager: now with two Arc references (one for auth_config, one for bot_config).
pub struct AuthManager {
    pub credentials_repo: Box<dyn CredentialsRepository + Send + Sync>,
    pub auth_config_repo: Arc<dyn AuthConfigRepository + Send + Sync>,
    pub bot_config_repo: Arc<dyn BotConfigRepository + Send + Sync>,

    pub authenticators: HashMap<Platform, Box<dyn PlatformAuthenticator + Send + Sync>>,
}

impl AuthManager {
    pub fn new(
        credentials_repo: Box<dyn CredentialsRepository + Send + Sync>,
        auth_config_repo: Arc<dyn AuthConfigRepository + Send + Sync>,
        bot_config_repo: Arc<dyn BotConfigRepository + Send + Sync>,
    ) -> Self {
        Self {
            credentials_repo,
            auth_config_repo,
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

    /// This older convenience method is left for backward-compat, but it always uses label="default".
    pub async fn authenticate_platform(
        &mut self,
        platform: Platform
    ) -> Result<PlatformCredential, Error> {
        self.authenticate_platform_for_role_label(platform, false, "default").await
    }

    /// Another older method with is_bot but fixed label="default".
    pub async fn authenticate_platform_for_role(
        &mut self,
        platform: Platform,
        is_bot: bool,
    ) -> Result<PlatformCredential, Error> {
        self.authenticate_platform_for_role_label(platform, is_bot, "default").await
    }

    /// New: same as above, but we accept a label param.
    /// This does a begin_auth_flow_with_label + then expects a code (which we won't have).
    /// We return an error because we want the TUI to handle the 2-step flow.
    pub async fn authenticate_platform_for_role_label(
        &mut self,
        platform: Platform,
        is_bot: bool,
        label: &str,
    ) -> Result<PlatformCredential, Error> {
        let _ = self.begin_auth_flow_with_label(platform.clone(), is_bot, label).await?;
        Err(Error::Auth("This function expects a code, but none was provided (use the 2-step flow)".into()))
    }

    /// New version: we pass in the label, so multiple client_id sets are possible.
    /// If no row is found, we return an Error::Auth(...) that says "No auth_config row found..."
    /// The TUI can catch that and create a row, then re-call this method.
    pub async fn begin_auth_flow_with_label(
        &mut self,
        platform: Platform,
        is_bot: bool,
        label: &str
    ) -> Result<String, Error> {
        let platform_str = match &platform {
            Platform::Twitch => "twitch",
            Platform::Discord => "discord",
            Platform::VRChat => "vrchat",
            Platform::TwitchIRC => "twitch-irc",
        };

        let maybe_row = self.auth_config_repo
            .get_by_platform_and_label(platform_str, label)
            .await?;

        let (client_id, client_secret) = if let Some(conf_row) = maybe_row {
            let cid = conf_row.client_id.unwrap_or_default();
            let csec = conf_row.client_secret;
            (cid, csec)
        } else {
            return Err(Error::Auth(format!(
                "No auth_config row found for platform={} label={}",
                platform_str, label
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
                    Arc::clone(&self.bot_config_repo),
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
                // If it’s a Browser{ url }, we return that
                match prompt {
                    AuthenticationPrompt::Browser { url } => {
                        return Ok(url);
                    }
                    AuthenticationPrompt::Code { message } => {
                        return Err(Error::Auth(message)); // or return the message as a “url”
                    }
                    AuthenticationPrompt::ApiKey { message } => {
                        return Ok(format!("(API key prompt) {}", message));
                    }
                    AuthenticationPrompt::MultipleKeys { .. } => {
                        // We could handle that in TUI, but for simplicity we treat it as an error message
                        return Ok("(Multiple keys required) please handle in TUI".into());
                    }
                    AuthenticationPrompt::TwoFactor { message } => {
                        return Ok(format!("(2FA) {}", message));
                    }
                    AuthenticationPrompt::None => {
                        return Ok("(No user prompt required)".into());
                    }
                }
            }
        }
        Err(Error::Platform(format!("Could not begin auth flow for label='{}'", label)))
    }

    /// Step 2: supply the code to finish OAuth.
    pub async fn complete_auth_flow(
        &mut self,
        platform: Platform,
        code: String,
    ) -> Result<PlatformCredential, Error> {
        let authenticator = self.authenticators
            .get_mut(&platform)
            .ok_or_else(|| Error::Platform(format!("No authenticator for {platform:?}")))?;

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

    /// NEW: create_auth_config => store a row in `auth_config` so the user doesn’t have to
    /// manually do it. TUI can call this after prompting for client_id/secret.
    pub async fn create_auth_config(
        &self,
        platform_str: &str,
        label: &str,
        client_id: String,
        client_secret: Option<String>,
    ) -> Result<(), Error> {
        // We'll just call the repository's insert method.
        // (Assuming `insert_auth_config` signature: insert_auth_config(platform, label, client_id, client_secret) -> Result<_, _>
        self.auth_config_repo.insert_auth_config(platform_str, label, client_id, client_secret).await?;
        Ok(())
    }

    /// NEW: returns how many auth_config rows exist for the given platform
    pub async fn count_auth_configs_for_platform(&self, platform_str: &str) -> Result<usize, Error> {
        let n = self.auth_config_repo.count_for_platform(platform_str).await?;
        Ok(n as usize)
    }
}