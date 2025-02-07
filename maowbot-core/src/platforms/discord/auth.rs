use async_trait::async_trait;
use chrono::Utc;
use serde_json::json;
use uuid::Uuid;

use crate::Error;
use crate::auth::{AuthenticationPrompt, AuthenticationResponse, PlatformAuthenticator};
use crate::models::{Platform, PlatformCredential, CredentialType};

pub struct DiscordAuthenticator {
    // We store them here after the AuthManager fetches from DB.
    client_id: Option<String>,
    client_secret: Option<String>,
    bot_token: Option<String>,
    is_bot: bool,
}

impl DiscordAuthenticator {
    /// Updated constructor: we receive optional client_id, client_secret from AuthManager
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
        // Possibly confirm that client_id is set if we need it, or do nothing
        Ok(())
    }

    async fn start_authentication(&mut self) -> Result<AuthenticationPrompt, Error> {
        // For a typical Discord bot, you actually only need the "bot token" from dev portal.
        // We'll mimic that with a multi-key prompt:
        Ok(AuthenticationPrompt::MultipleKeys {
            fields: vec![
                "bot_token".into()
            ],
            messages: vec![
                "Enter your Discord Bot Token".into()
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

                let bot_token = self.bot_token.as_ref()
                    .ok_or_else(|| Error::Auth("Bot token is required".into()))?;

                Ok(PlatformCredential {
                    credential_id: Uuid::new_v4().to_string(),
                    platform: Platform::Discord,
                    credential_type: CredentialType::BearerToken,
                    user_id: String::new(), // set later
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
                })
            }
            _ => Err(Error::Auth("Invalid authentication response for Discord".into())),
        }
    }

    async fn refresh(&mut self, credential: &PlatformCredential) -> Result<PlatformCredential, Error> {
        // Discord bot tokens typically don't need refreshing
        Ok(credential.clone())
    }

    async fn validate(&self, credential: &PlatformCredential) -> Result<bool, Error> {
        // Example logic: call Discord's API to validate the token
        use reqwest::Client;

        let client = Client::new();
        let response = client.get("https://discord.com/api/v10/users/@me")
            .header("Authorization", format!("Bot {}", credential.primary_token))
            .send()
            .await?;

        Ok(response.status().is_success())
    }

    async fn revoke(&mut self, _credential: &PlatformCredential) -> Result<(), Error> {
        // Bot tokens can't be revoked programmatically; user must regenerate in the portal
        Ok(())
    }

    fn set_is_bot(&mut self, val: bool) {
        self.is_bot = val;
    }
}