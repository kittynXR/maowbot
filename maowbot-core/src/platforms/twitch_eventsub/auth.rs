use async_trait::async_trait;
use chrono::Utc;
use uuid::Uuid;
use serde_json::json;

use crate::Error;
use crate::auth::{AuthenticationPrompt, AuthenticationResponse, PlatformAuthenticator};
use crate::models::{Platform, PlatformCredential, CredentialType};

/// TwitchEventSubAuthenticator is now just a stub; we no longer prompt or do OAuth here.
/// The TUI/DB logic re-uses the Twitch Helix token for eventsub.
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

    // CHANGED: Always returns "None" because we do not do a separate flow here.
    async fn start_authentication(&mut self) -> Result<AuthenticationPrompt, Error> {
        Ok(AuthenticationPrompt::None)
    }

    // CHANGED: We error out if someone tries to complete an auth flow directly for EventSub,
    // because we expect to re-use Helix credentials from the TUI code.
    async fn complete_authentication(
        &mut self,
        _response: AuthenticationResponse,
    ) -> Result<PlatformCredential, Error> {
        Err(Error::Auth(
            "EventSub re-uses Helix tokens; no direct OAuth required.".into()
        ))
    }

    async fn refresh(&mut self, credential: &PlatformCredential) -> Result<PlatformCredential, Error> {
        // For this stub, assume the token never expires or is handled by Helix refresh.
        Ok(credential.clone())
    }

    async fn validate(&self, _credential: &PlatformCredential) -> Result<bool, Error> {
        // Stub: always valid, or you can optionally implement real checks.
        Ok(true)
    }

    async fn revoke(&mut self, _credential: &PlatformCredential) -> Result<(), Error> {
        // Stub: nothing to revoke for EventSub (the Helix token revocation covers it).
        Ok(())
    }

    fn set_is_bot(&mut self, val: bool) {
        self.is_bot = val;
    }
}
