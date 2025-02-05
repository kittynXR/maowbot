use async_trait::async_trait;
use chrono::Utc;
use uuid::Uuid;
use serde_json::json;

use crate::auth::{AuthenticationPrompt, AuthenticationResponse, PlatformAuthenticator};
use crate::models::{Platform, PlatformCredential, CredentialType};
use crate::Error;

/// Example `TwitchIrcAuthenticator`.
///
/// Twitch IRC itself can accept an “OAuth token” (often a special chat token
/// that starts with `oauth:`) for chat-based authentication.
/// The user might get it from https://twitchapps.com/tmi/
/// or from their official “Chat OAuth” endpoint.
pub struct TwitchIrcAuthenticator {
    // Possibly store any config needed.
    pub chat_oauth_url: String,
    pub token_hint: Option<String>,
}

impl TwitchIrcAuthenticator {
    pub fn new() -> Self {
        Self {
            chat_oauth_url: "https://twitchapps.com/tmi/".to_string(),
            token_hint: None,
        }
    }
}

#[async_trait]
impl PlatformAuthenticator for TwitchIrcAuthenticator {
    async fn initialize(&mut self) -> Result<(), Error> {
        Ok(())
    }

    async fn start_authentication(&mut self) -> Result<AuthenticationPrompt, Error> {
        // We can direct them to open a site that obtains a Twitch "chat token".
        // Usually, the user obtains something like: "oauth:abcdefgh..."
        Ok(AuthenticationPrompt::ApiKey {
            message: format!(
                "Please get a chat token from {} and paste it. Format: oauth:xxxxx",
                self.chat_oauth_url
            ),
        })
    }

    async fn complete_authentication(
        &mut self,
        response: AuthenticationResponse
    ) -> Result<PlatformCredential, Error> {
        let token = match response {
            AuthenticationResponse::ApiKey(key) => key,
            _ => return Err(Error::Auth("Expected an API key for Twitch IRC".into())),
        };
        // Validate it’s in the right format? Optionally do a quick check if it starts with "oauth:"?
        if !token.starts_with("oauth:") {
            return Err(Error::Auth("Twitch IRC token must begin with 'oauth:'".into()));
        }

        Ok(PlatformCredential {
            credential_id: Uuid::new_v4().to_string(),
            platform: Platform::TwitchIRC,
            credential_type: CredentialType::BearerToken, // or CredentialType::APIKey
            user_id: String::new(),
            primary_token: token,
            refresh_token: None,
            additional_data: Some(json!({ "source": "twitch-irc-token" })),
            expires_at: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            is_bot: true,
        })
    }

    async fn refresh(&mut self, credential: &PlatformCredential) -> Result<PlatformCredential, Error> {
        // For IRC tokens, we typically can't “refresh.” Usually user must re-obtain one.
        Ok(credential.clone())
    }

    async fn validate(&self, credential: &PlatformCredential) -> Result<bool, Error> {
        // A minimal check could be “does it start with 'oauth:'?”
        // Or we can connect to IRC and see if PASS <token> succeeds.
        // For simplicity:
        if credential.primary_token.starts_with("oauth:") {
            Ok(true)
        } else {
            Ok(false)
        }
    }

    async fn revoke(&mut self, _credential: &PlatformCredential) -> Result<(), Error> {
        // If the user wants to revoke, they'd basically just not use that token in chat anymore.
        // There's no official revoke for Twitch IRC tokens. So we do a no-op:
        Ok(())
    }
}
