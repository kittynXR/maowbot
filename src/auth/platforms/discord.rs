// src/auth/platforms/discord.rs
use crate::auth::{AuthenticationPrompt, AuthenticationResponse, PlatformAuthenticator};
use crate::models::{Platform, PlatformCredential, CredentialType};
use crate::Error;
use async_trait::async_trait;
use chrono::Utc;
use serde_json::json;
use uuid::Uuid;

pub struct DiscordAuthenticator {
    client_id: Option<String>,
    client_secret: Option<String>,
    bot_token: Option<String>,
}

impl DiscordAuthenticator {
    pub fn new() -> Self {
        Self {
            client_id: None,
            client_secret: None,
            bot_token: None,
        }
    }
}

#[async_trait]
impl PlatformAuthenticator for DiscordAuthenticator {
    async fn initialize(&mut self) -> Result<(), Error> {
        Ok(())
    }

    async fn start_authentication(&mut self) -> Result<AuthenticationPrompt, Error> {
        Ok(AuthenticationPrompt::MultipleKeys {
            fields: vec![
                "client_id".into(),
                "client_secret".into(),
                "bot_token".into()
            ],
            messages: vec![
                "Enter your Discord Application Client ID".into(),
                "Enter your Discord Application Client Secret".into(),
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
                self.client_id = keys.get("client_id").cloned();
                self.client_secret = keys.get("client_secret").cloned();
                self.bot_token = keys.get("bot_token").cloned();

                let bot_token = self.bot_token.as_ref()
                    .ok_or_else(|| Error::Auth("Bot token is required".into()))?;

                // Create credential with bot token
                Ok(PlatformCredential {
                    credential_id: Uuid::new_v4().to_string(),
                    platform: Platform::Discord,
                    credential_type: CredentialType::BearerToken,
                    user_id: String::new(), // Will be populated after validation
                    primary_token: bot_token.clone(),
                    refresh_token: None,
                    additional_data: Some(json!({
                        "client_id": self.client_id,
                        "client_secret": self.client_secret
                    })),
                    expires_at: None, // Bot tokens don't expire
                    created_at: Utc::now().naive_utc(),
                    updated_at: Utc::now().naive_utc(),
                })
            }
            _ => Err(Error::Auth("Invalid authentication response".into())),
        }
    }

    async fn refresh(&mut self, credential: &PlatformCredential) -> Result<PlatformCredential, Error> {
        // Discord bot tokens don't need refresh
        Ok(credential.clone())
    }

    async fn validate(&self, credential: &PlatformCredential) -> Result<bool, Error> {
        // Here you would typically make a test API call to Discord
        // to verify the token is valid
        use reqwest::Client;

        let client = Client::new();
        let response = client.get("https://discord.com/api/v10/users/@me")
            .header("Authorization", format!("Bot {}", credential.primary_token))
            .send()
            .await?;

        Ok(response.status().is_success())
    }

    async fn revoke(&mut self, _credential: &PlatformCredential) -> Result<(), Error> {
        // Discord bot tokens can't be revoked - they need to be regenerated
        // in the Discord Developer Portal
        Ok(())
    }
}