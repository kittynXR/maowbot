use async_trait::async_trait;
use chrono::Utc;
use serde_json::json;
use uuid::Uuid;

use crate::Error;
use crate::auth::{AuthenticationPrompt, AuthenticationResponse, PlatformAuthenticator};
use crate::models::{Platform, PlatformCredential, CredentialType};

pub struct DiscordAuthenticator {
    // These values will be provided (or fetched from DB) by the AuthManager.
    client_id: Option<String>,
    client_secret: Option<String>,
    bot_token: Option<String>,
    is_bot: bool,
}

impl DiscordAuthenticator {
    /// Updated constructor: receives optional client_id and client_secret.
    pub fn new(
        client_id: Option<String>,
        client_secret: Option<String>,
    ) -> Self {
        Self {
            client_id,
            client_secret,
            bot_token: None,
            is_bot: false,
        }
    }
}

#[async_trait]
impl PlatformAuthenticator for DiscordAuthenticator {
    async fn initialize(&mut self) -> Result<(), Error> {
        // Nothing to initialize.
        Ok(())
    }

    async fn start_authentication(&mut self) -> Result<AuthenticationPrompt, Error> {
        // Updated to require three keys: the bot token, the bot's user id, and the bot's username.
        Ok(AuthenticationPrompt::MultipleKeys {
            fields: vec![
                "bot_token".into(),
                "bot_user_id".into(),
                "bot_username".into(),
            ],
            messages: vec![
                "Enter your Discord Bot Token".into(),
                "Enter your Discord Bot User ID".into(),
                "Enter your Discord Bot Username".into(),
            ],
        })
    }

    async fn complete_authentication(
        &mut self,
        response: AuthenticationResponse,
    ) -> Result<PlatformCredential, Error> {
        match response {
            AuthenticationResponse::MultipleKeys(keys) => {
                self.bot_token = keys.get("bot_token").cloned();
                let bot_user_id = keys.get("bot_user_id")
                    .cloned()
                    .ok_or_else(|| Error::Auth("Bot user ID is required".into()))?;
                let bot_username = keys.get("bot_username")
                    .cloned()
                    .ok_or_else(|| Error::Auth("Bot username is required".into()))?;

                let bot_token = self.bot_token.as_ref()
                    .ok_or_else(|| Error::Auth("Bot token is required".into()))?;

                Ok(PlatformCredential {
                    credential_id: Uuid::new_v4(),
                    platform: Platform::Discord,
                    credential_type: CredentialType::BearerToken,
                    user_id: Uuid::new_v4(), // This will be updated later by the AuthManager
                    primary_token: bot_token.clone(),
                    refresh_token: None,
                    additional_data: Some(json!({
                        "client_id": self.client_id,
                        "client_secret": self.client_secret
                    })),
                    expires_at: None,
                    created_at: Utc::now(),
                    updated_at: Utc::now(),
                    is_bot: self.is_bot,
                    // NEW fields:
                    platform_id: Some(bot_user_id),
                    user_name: bot_username,
                })
            }
            _ => Err(Error::Auth("Invalid authentication response for Discord".into())),
        }
    }

    async fn refresh(&mut self, credential: &PlatformCredential) -> Result<PlatformCredential, Error> {
        // Discord bot tokens typically do not expire.
        Ok(credential.clone())
    }

    async fn validate(&self, credential: &PlatformCredential) -> Result<bool, Error> {
        use reqwest::Client;
        let client = Client::new();
        let response = client.get("https://discord.com/api/v10/users/@me")
            .header("Authorization", format!("Bot {}", credential.primary_token))
            .send()
            .await?;
        Ok(response.status().is_success())
    }

    async fn revoke(&mut self, _credential: &PlatformCredential) -> Result<(), Error> {
        // Bot tokens cannot be revoked programmatically.
        Ok(())
    }

    fn set_is_bot(&mut self, val: bool) {
        self.is_bot = val;
    }
}