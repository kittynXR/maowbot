// maowbot-core/src/auth/manager.rs
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;
use maowbot_common::error::Error;

use maowbot_common::traits::auth_traits::PlatformAuthenticator;
use maowbot_common::models::platform::{Platform, PlatformCredential};
use maowbot_common::traits::repository_traits::{BotConfigRepository, CredentialsRepository};
use crate::auth::{AuthenticationPrompt, AuthenticationResponse};
use crate::repositories::postgres::platform_config::PlatformConfigRepository;

use crate::platforms::discord::auth::DiscordAuthenticator;
use crate::platforms::twitch_eventsub::TwitchEventSubAuthenticator;
use crate::platforms::twitch::auth::TwitchAuthenticator;
use crate::platforms::vrchat::auth::VRChatAuthenticator;
use crate::platforms::twitch_irc::auth::TwitchIrcAuthenticator;

pub struct AuthManager {
    pub credentials_repo: Arc<dyn CredentialsRepository + Send + Sync>,
    pub platform_config_repo: Arc<dyn PlatformConfigRepository + Send + Sync>,
    pub bot_config_repo: Arc<dyn BotConfigRepository + Send + Sync>,
    pub authenticators: HashMap<Platform, Box<dyn PlatformAuthenticator + Send + Sync>>,
}

impl AuthManager {
    pub fn new(
        credentials_repo: Arc<dyn CredentialsRepository + Send + Sync>,
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

    /// A simpler helper: always do the entire “build or retrieve from map” in one pass,
    /// then return. Notice we do *not* return `&mut Box<...>`.
    async fn ensure_authenticator_exists(
        &mut self,
        platform: &Platform,
    ) -> Result<(), Error> {
        if self.authenticators.contains_key(platform) {
            // Already there, just return
            return Ok(());
        }

        // Look up client_id/secret from DB
        let platform_str = platform.to_string();
        let maybe_conf = self.platform_config_repo.get_by_platform(&platform_str).await?;
        let conf_row = match maybe_conf {
            Some(row) => row,
            None => {
                return Err(Error::Auth(format!(
                    "No platform_config found for platform='{}'",
                    platform_str
                )));
            }
        };

        let client_id = conf_row.client_id.unwrap_or_default();
        let client_secret = conf_row.client_secret;

        // Build the authenticator
        let mut new_auth: Box<dyn PlatformAuthenticator + Send + Sync> = match platform {
            Platform::Discord => Box::new(DiscordAuthenticator::new(Some(client_id), client_secret)),
            Platform::Twitch => Box::new(TwitchAuthenticator::new(client_id, client_secret)),
            Platform::VRChat => Box::new(VRChatAuthenticator::new()),
            Platform::TwitchIRC => Box::new(TwitchIrcAuthenticator::new(client_id, client_secret)),
            Platform::TwitchEventSub => Box::new(TwitchEventSubAuthenticator::new(client_id, client_secret)),
            Platform::OBS => {
                // OBS doesn't use OAuth, so we can't create an authenticator this way
                return Err(Error::Platform("OBS does not use OAuth authentication".into()));
            }
        };

        new_auth.initialize().await?;
        self.authenticators.insert(platform.clone(), new_auth);
        Ok(())
    }

    // --------------------------
    // OAuth flows
    // --------------------------

    pub async fn begin_auth_flow(
        &mut self,
        platform: Platform,
        is_bot: bool,
    ) -> Result<String, Error> {
        // fetch config from DB
        let platform_str = platform.to_string();
        let maybe_conf = self.platform_config_repo.get_by_platform(&platform_str).await?;
        let (client_id, client_secret) = if let Some(conf_row) = maybe_conf {
            (conf_row.client_id.unwrap_or_default(), conf_row.client_secret)
        } else {
            return Err(Error::Auth(format!(
                "No platform_config found for platform={}",
                platform_str
            )));
        };

        // create the authenticator, insert in map
        let mut authenticator: Box<dyn PlatformAuthenticator + Send + Sync> = match platform {
            Platform::Discord => Box::new(DiscordAuthenticator::new(Some(client_id), client_secret)),
            Platform::Twitch => Box::new(TwitchAuthenticator::new(client_id, client_secret)),
            Platform::VRChat => Box::new(VRChatAuthenticator::new()),
            Platform::TwitchIRC => Box::new(TwitchIrcAuthenticator::new(client_id, client_secret)),
            Platform::TwitchEventSub => Box::new(TwitchEventSubAuthenticator::new(client_id, client_secret)),
            Platform::OBS => {
                // OBS doesn't use OAuth, so we can't create an authenticator this way
                return Err(Error::Platform("OBS does not use OAuth authentication".into()));
            }
        };
        authenticator.set_is_bot(is_bot);
        authenticator.initialize().await?;

        // store in our HashMap
        self.authenticators.insert(platform.clone(), authenticator);

        // now do `start_authentication` in a short scope
        let prompt = {
            let auth = self.authenticators.get_mut(&platform).unwrap();
            auth.start_authentication().await?
        };
        match prompt {
            AuthenticationPrompt::Browser { url } => Ok(url),
            AuthenticationPrompt::Code { message } => Err(Error::Auth(message)),
            AuthenticationPrompt::ApiKey { message } => Ok(format!("(API key) {message}")),
            // CHANGE HERE to match the TUI check for "MultipleKeys"
            AuthenticationPrompt::MultipleKeys { .. } => Ok("(MultipleKeys) handle in TUI".into()),
            AuthenticationPrompt::TwoFactor { message } => Ok(format!("(2FA) {message}")),
            AuthenticationPrompt::None => Ok("(No prompt needed)".into()),
        }
    }

    pub async fn complete_auth_flow_for_user(
        &mut self,
        platform: Platform,
        code: String,
        user_id: &str,
    ) -> Result<PlatformCredential, Error> {
        let Some(auth) = self.authenticators.get_mut(&platform) else {
            return Err(Error::Platform(format!("No authenticator for {platform:?}")));
        };

        let mut cred = auth
            .complete_authentication(AuthenticationResponse::Code(code))
            .await?;
        let user_uuid = Uuid::parse_str(user_id)
            .map_err(|e| Error::Auth(format!("Bad user_id: {e}")))?;
        cred.user_id = user_uuid;
        self.credentials_repo.store_credentials(&cred).await?;
        Ok(cred)
    }

    pub async fn complete_auth_flow_for_user_multi(
        &mut self,
        platform: Platform,
        user_id: &Uuid,
        keys: HashMap<String, String>,
    ) -> Result<PlatformCredential, Error> {
        let Some(auth) = self.authenticators.get_mut(&platform) else {
            return Err(Error::Platform(format!("No authenticator for {platform:?}")));
        };
        let mut cred = auth
            .complete_authentication(AuthenticationResponse::MultipleKeys(keys))
            .await?;

        cred.user_id = *user_id;
        self.credentials_repo.store_credentials(&cred).await?;
        Ok(cred)
    }

    pub async fn complete_auth_flow_for_user_twofactor(
        &mut self,
        platform: Platform,
        code: String,
        user_id: &Uuid
    ) -> Result<PlatformCredential, Error> {
        let Some(auth) = self.authenticators.get_mut(&platform) else {
            return Err(Error::Platform(format!("No authenticator for {platform:?}")));
        };

        let mut cred = auth
            .complete_authentication(AuthenticationResponse::TwoFactor(code))
            .await?;

        cred.user_id = *user_id;
        self.credentials_repo.store_credentials(&cred).await?;
        Ok(cred)
    }

    // --------------------------
    // Revoke / Refresh
    // --------------------------

    pub async fn revoke_credentials(
        &mut self,
        platform: &Platform,
        user_id: &str,
    ) -> Result<(), Error> {
        let user_uuid = match Uuid::parse_str(user_id) {
            Ok(u) => u,
            Err(e) => return Err(Error::Auth(format!("Cannot parse user_id as UUID: {e}"))),
        };

        let cred_opt = self.credentials_repo.get_credentials(platform, user_uuid).await?;
        if cred_opt.is_none() {
            return Ok(()); // nothing to revoke
        }
        let cred = cred_opt.unwrap();

        // 1) ensure authenticator is loaded
        self.ensure_authenticator_exists(platform).await?;

        // 2) do revoke in a short scope
        {
            let auth = self.authenticators.get_mut(platform).unwrap();
            auth.revoke(&cred).await?;
        }
        // 3) now we can delete from DB
        self.credentials_repo.delete_credentials(platform, user_uuid).await?;
        Ok(())
    }

    pub async fn refresh_platform_credentials(
        &mut self,
        platform: &Platform,
        user_id: &Uuid,
    ) -> Result<PlatformCredential, Error> {
        let cred_opt = self.credentials_repo.get_credentials(platform, *user_id).await?;
        let Some(old_cred) = cred_opt else {
            return Err(Error::Auth("No credentials found".into()));
        };

        // ensure authenticator is loaded
        self.ensure_authenticator_exists(platform).await?;

        let new_cred = {
            let auth = self.authenticators.get_mut(platform).unwrap();
            auth.refresh(&old_cred).await?
        };
        self.credentials_repo.store_credentials(&new_cred).await?;
        Ok(new_cred)
    }

    // --------------------------
    // Utility lookups
    // --------------------------

    pub async fn store_credentials(&self, cred: &PlatformCredential) -> Result<(), Error> {
        self.credentials_repo.store_credentials(cred).await
    }

    pub async fn validate_credentials(
        &mut self,
        cred: &PlatformCredential
    ) -> Result<bool, Error> {
        self.ensure_authenticator_exists(&cred.platform).await?;
        let auth = self.authenticators.get_mut(&cred.platform).unwrap();
        auth.validate(cred).await
    }

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
