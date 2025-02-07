use std::collections::HashMap;
use std::sync::Arc;
use crate::auth::{PlatformAuthenticator, AuthenticationPrompt, AuthenticationResponse};
use crate::Error;
use crate::models::{Platform, PlatformCredential};
use crate::repositories::{BotConfigRepository, CredentialsRepository};
use crate::repositories::postgres::auth_config::AuthConfigRepository;

// import your platform authenticators:
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

    /// Register an authenticator for a given platform (if you want static config).
    /// But typically we'll create authenticators on-demand in `begin_auth_flow`.
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

    pub async fn authenticate_platform_for_role(
        &mut self,
        platform: Platform,
        is_bot: bool,
    ) -> Result<PlatformCredential, Error> {
        // We keep this for older code, but it triggers an error in the new approach.
        let _ = self.begin_auth_flow(platform.clone(), is_bot).await?;
        Err(Error::Auth("This function expects a code, but none was provided (use begin_auth_flow and complete_auth_flow).".into()))
    }

    /// Step 1 of the interactive flow: fetch client_id/client_secret from DB
    /// (via auth_config_repo), build the authenticator, store it in `self.authenticators`.
    pub async fn begin_auth_flow(
        &mut self,
        platform: Platform,
        is_bot: bool,
    ) -> Result<AuthenticationPrompt, Error> {
        // Example: just pick a label = "default"
        let label = "default";

        let platform_str = match &platform {
            Platform::Twitch => "twitch",
            Platform::Discord => "discord",
            Platform::VRChat => "vrchat",
            Platform::TwitchIRC => "twitch-irc",
        };

        // Attempt to find a row in `auth_config`:
        let maybe_row = self.auth_config_repo.get_by_platform_and_label(platform_str, label).await?;
        let (client_id, client_secret) = if let Some(conf_row) = maybe_row {
            // If row has no secret, that's fine => PKCE only
            let cid = conf_row.client_id.unwrap_or_default();
            let csec = conf_row.client_secret;
            (cid, csec)
        } else {
            return Err(Error::Auth(format!("No auth_config row found for platform={platform_str} label={label}")));
        };

        // Construct the platform authenticator:
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
                    Arc::clone(&self.bot_config_repo),  // pass it so we can get callback_port
                ))
            }
            Platform::VRChat => {
                Box::new(VRChatAuthenticator::new())
            }
            Platform::TwitchIRC => {
                Box::new(TwitchIrcAuthenticator::new())
            }
        };

        // Insert into map
        self.authenticators.insert(platform.clone(), authenticator);

        // set is_bot
        if let Some(auth) = self.authenticators.get_mut(&platform) {
            auth.set_is_bot(is_bot);
            auth.initialize().await?;
            let prompt = auth.start_authentication().await?;
            Ok(prompt)
        } else {
            Err(Error::Platform("Could not insert or fetch authenticator".into()))
        }
    }

    /// Step 2: supply the code, do the token exchange
    pub async fn complete_auth_flow(
        &mut self,
        platform: Platform,
        code: String,
    ) -> Result<PlatformCredential, Error> {
        let authenticator = self.authenticators.get_mut(&platform)
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