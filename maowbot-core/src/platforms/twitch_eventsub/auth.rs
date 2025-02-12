// File: maowbot-core/src/platforms/eventsub/auth.rs

use async_trait::async_trait;
use chrono::Utc;
use uuid::Uuid;
use serde_json::json;
use crate::Error;
use crate::auth::{AuthenticationPrompt, AuthenticationResponse, PlatformAuthenticator};
use crate::models::{Platform, PlatformCredential, CredentialType};

/// TwitchEventSubAuthenticator is a stub for handling EventSub authentication.
/// In this simple implementation we re-use the Twitch Helix OAuth token.
/// In a full implementation you might have a dedicated OAuth flow if needed.
pub struct TwitchEventSubAuthenticator {
    pub client_id: String,
    pub client_secret: Option<String>,
    pub helix_token: Option<String>,
    pub is_bot: bool,
}

impl TwitchEventSubAuthenticator {
    pub fn new(client_id: String, client_secret: Option<String>) -> Self {
        Self {
            client_id,
            client_secret,
            helix_token: None,
            is_bot: false,
        }
    }
}

#[async_trait]
impl PlatformAuthenticator for TwitchEventSubAuthenticator {
    async fn initialize(&mut self) -> Result<(), Error> {
        // Nothing extra to initialize.
        Ok(())
    }

    async fn start_authentication(&mut self) -> Result<AuthenticationPrompt, Error> {
        // For EventSub we assume the user should supply the same helix token.
        Ok(AuthenticationPrompt::ApiKey {
            message: "Please enter your Twitch Helix OAuth token (this will be re-used for EventSub)".into(),
        })
    }

    async fn complete_authentication(
        &mut self,
        response: AuthenticationResponse,
    ) -> Result<PlatformCredential, Error> {
        let token = match response {
            AuthenticationResponse::ApiKey(key) => key,
            _ => return Err(Error::Auth("Expected API key for EventSub".into())),
        };
        self.helix_token = Some(token.clone());
        let now = Utc::now();
        Ok(PlatformCredential {
            credential_id: Uuid::new_v4(),
            platform: Platform::TwitchEventSub,
            credential_type: CredentialType::OAuth2,
            user_id: Uuid::new_v4(), // This will be updated later
            primary_token: token,    // Re-use the helix token
            refresh_token: None,
            additional_data: Some(json!({
                "note": "EventSub uses the same token as Twitch Helix"
            })),
            expires_at: None,
            created_at: now,
            updated_at: now,
            is_bot: self.is_bot,
        })
    }

    async fn refresh(&mut self, credential: &PlatformCredential) -> Result<PlatformCredential, Error> {
        // For this stub, assume the token never expires or does not need refreshing.
        Ok(credential.clone())
    }

    async fn validate(&self, _credential: &PlatformCredential) -> Result<bool, Error> {
        // Stub: always valid.
        Ok(true)
    }

    async fn revoke(&mut self, _credential: &PlatformCredential) -> Result<(), Error> {
        // Stub: nothing to revoke.
        Ok(())
    }

    fn set_is_bot(&mut self, val: bool) {
        self.is_bot = val;
    }
}
