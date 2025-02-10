use std::collections::HashMap;
use std::sync::Arc;
use clap::builder::TypedValueParser;
use uuid::Uuid;

use crate::auth::{PlatformAuthenticator, AuthenticationPrompt, AuthenticationResponse};
use crate::Error;
use crate::models::{Platform, PlatformCredential};
use crate::repositories::{BotConfigRepository, CredentialsRepository};
use crate::repositories::postgres::platform_config::PlatformConfigRepository;

use crate::platforms::discord::auth::DiscordAuthenticator;
use crate::platforms::twitch_helix::auth::TwitchAuthenticator;
use crate::platforms::vrchat::auth::VRChatAuthenticator;
use crate::platforms::twitch_irc::auth::TwitchIrcAuthenticator;

/// AuthManager: manages platform authenticators, reading config from DB
/// and storing credentials in DB once retrieved.
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

    pub fn register_authenticator(
        &mut self,
        platform: Platform,
        authenticator: Box<dyn PlatformAuthenticator + Send + Sync + 'static>,
    ) {
        self.authenticators.insert(platform, authenticator);
    }

    /// For older usage: calls begin_auth_flow but never completes it properly
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

    /// Step 1 of OAuth or other auth: get the redirect URL or instructions
    /// Note that for Discord, it returns something like "(Multiple keys required) handle in TUI".
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

        // get config from DB
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

        // create the authenticator for the requested platform
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
            match auth.start_authentication().await? {
                AuthenticationPrompt::Browser { url } => Ok(url),
                AuthenticationPrompt::Code { message } => Err(Error::Auth(message)),
                AuthenticationPrompt::ApiKey { message } => {
                    Ok(format!("(API key) {message}"))
                }
                AuthenticationPrompt::MultipleKeys { .. } => {
                    Ok("(Multiple keys required) handle in TUI".into())
                }
                AuthenticationPrompt::TwoFactor { message } => Ok(format!("(2FA) {message}")),
                AuthenticationPrompt::None => Ok("(No prompt needed)".into()),
            }
        } else {
            Err(Error::Platform(format!(
                "No authenticator available for platform={platform:?}"
            )))
        }
    }

    /// Step 2 (old usage): tries to complete for the given code, but sets user_id = ""
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

        // This fails if user_id="" is not found in DB:
        self.credentials_repo.store_credentials(&cred).await?;
        Ok(cred)
    }

    /// Step 2 (improved): specify the user_id (UUID string) to store in DB
    pub async fn complete_auth_flow_for_user(
        &mut self,
        platform: Platform,
        code: String,
        user_id: &str,
    ) -> Result<PlatformCredential, Error> {
        let authenticator = self
            .authenticators
            .get_mut(&platform)
            .ok_or_else(|| Error::Platform(format!("No authenticator for {platform:?}")))?;

        // finish the OAuth steps
        let mut cred = authenticator
            .complete_authentication(AuthenticationResponse::Code(code))
            .await?;

        // parse the user_id as a UUID
        let user_uuid = Uuid::parse_str(user_id)
            .map_err(|e| Error::Auth(format!("Failed to parse user_id as UUID: {e}")))?;

        // store the real UUID in the credential
        cred.user_id = user_uuid;

        // now persist to DB
        self.credentials_repo.store_credentials(&cred).await?;
        Ok(cred)
    }

    // -------------------------------------------------------------------------
    // ADDED: new method to handle "MultipleKeys" usage from the TUI. E.g. Discord bot_token
    // -------------------------------------------------------------------------
    pub async fn complete_auth_flow_for_user_multi(
        &mut self,
        platform: Platform,
        user_id: &Uuid,
        keys: HashMap<String, String>,
    ) -> Result<PlatformCredential, Error> {
        let authenticator = self
            .authenticators
            .get_mut(&platform)
            .ok_or_else(|| Error::Platform(format!("No authenticator for {platform:?}")))?;

        let mut cred = authenticator
            .complete_authentication(AuthenticationResponse::MultipleKeys(keys))
            .await?;

        cred.user_id = *user_id;
        self.credentials_repo.store_credentials(&cred).await?;
        Ok(cred)
    }
    // -------------------------------------------------------------------------

    /// If an external caller has already built a PlatformCredential, store it
    pub async fn store_credentials(&self, cred: &PlatformCredential) -> Result<(), Error> {
        self.credentials_repo.store_credentials(cred).await
    }

    /// Retrieve credentials
    pub async fn get_credentials(
        &self,
        platform: &Platform,
        user_id: &str,
    ) -> Result<Option<PlatformCredential>, Error> {
        self.credentials_repo
            .get_credentials(platform, user_id.parse().unwrap())
            .await
    }

    /// Revoke & remove from DB
    pub async fn revoke_credentials(
        &mut self,
        platform: &Platform,
        user_id: &str,
    ) -> Result<(), Error> {
        if let Some(cred) = self.credentials_repo.get_credentials(platform, user_id.parse().unwrap()).await? {
            let authenticator = self.authenticators.get_mut(platform)
                .ok_or_else(|| Error::Platform(format!("No authenticator for {platform:?}")))?;
            authenticator.revoke(&cred).await?;
            self.credentials_repo.delete_credentials(platform, user_id.parse().unwrap()).await?;
        }
        Ok(())
    }

    /// Attempt to refresh a credential
    pub async fn refresh_platform_credentials(
        &mut self,
        platform: &Platform,
        user_id: &Uuid,
    ) -> Result<PlatformCredential, Error> {
        let Some(cred) = self.credentials_repo
            .get_credentials(platform, *user_id).await? else {
            return Err(Error::Auth("No credentials found".into()));
        };

        let authenticator = self.authenticators.get_mut(platform)
            .ok_or_else(|| Error::Platform(format!("No authenticator for {platform:?}")))?;

        let refreshed = authenticator.refresh(&cred).await?;
        self.credentials_repo.store_credentials(&refreshed).await?;
        Ok(refreshed)
    }

    /// Confirm validity
    pub async fn validate_credentials(
        &mut self,
        cred: &PlatformCredential
    ) -> Result<bool, Error> {
        let authenticator = self.authenticators.get_mut(&cred.platform)
            .ok_or_else(|| Error::Platform(format!("No authenticator for {:?}", cred.platform)))?;
        authenticator.validate(cred).await
    }

    /// Upsert a platform_config record in DB
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