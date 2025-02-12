use async_trait::async_trait;
use chrono::{Utc, Duration};
use reqwest::Client as ReqwestClient;
use serde::Deserialize;
use std::sync::atomic::{AtomicUsize, Ordering};
use tracing::error;
use uuid::Uuid;

use crate::auth::{AuthenticationPrompt, AuthenticationResponse, PlatformAuthenticator};
use crate::models::{Platform, PlatformCredential, CredentialType};
use crate::Error;

/// JSON response from Twitch’s OAuth token endpoint.
#[derive(Deserialize)]
struct TwitchTokenResponse {
    access_token: String,
    refresh_token: Option<String>,
    expires_in: u64,
    scope: Option<Vec<String>>,
    token_type: String,
}

/// A simple static for unique `state` each time we begin an IRC authentication.
static IRC_STATE_COUNTER: AtomicUsize = AtomicUsize::new(0);

/// `TwitchIrcAuthenticator` uses an OAuth code flow similar to the Helix approach,
/// but specifically for IRC scopes (chat:read, chat:edit). We store `primary_token` as
/// `"oauth:xxxxx"` so that it is usable in the IRC PASS command.
pub struct TwitchIrcAuthenticator {
    pub client_id: String,
    pub client_secret: Option<String>,
    pub is_bot: bool,
    pending_state: Option<String>,
}

impl TwitchIrcAuthenticator {
    pub fn new(client_id: String, client_secret: Option<String>) -> Self {
        Self {
            client_id,
            client_secret,
            is_bot: true,
            pending_state: None,
        }
    }

    /// Build an authorize URL requesting `chat:read` and `chat:edit` scopes.
    fn build_auth_url(&self, state: &str) -> String {
        let scopes = vec!["chat:read", "chat:edit"];
        let scope_str = scopes.join(" ");
        let redirect_uri = "http://localhost:9876/callback";

        format!(
            "https://id.twitch.tv/oauth2/authorize?response_type=code&client_id={cid}\
             &redirect_uri={redir}&scope={scope}&state={st}",
            cid   = urlencoding::encode(&self.client_id),
            redir = urlencoding::encode(redirect_uri),
            scope = urlencoding::encode(&scope_str),
            st    = urlencoding::encode(state),
        )
    }
}

#[async_trait]
impl PlatformAuthenticator for TwitchIrcAuthenticator {
    async fn initialize(&mut self) -> Result<(), Error> {
        Ok(())
    }

    async fn start_authentication(&mut self) -> Result<AuthenticationPrompt, Error> {
        let c = IRC_STATE_COUNTER.fetch_add(1, Ordering::SeqCst);
        let state = format!("irc-state-{}", c);

        // build the URL for the user
        let auth_url = self.build_auth_url(&state);
        self.pending_state = Some(state);

        Ok(AuthenticationPrompt::Browser { url: auth_url })
    }

    async fn complete_authentication(
        &mut self,
        response: AuthenticationResponse
    ) -> Result<PlatformCredential, Error> {
        // Must be a code flow:
        let code = match response {
            AuthenticationResponse::Code(c) => c,
            _ => return Err(Error::Auth("Expected code in IRC flow".into())),
        };

        // Exchange code -> access_token
        let http = ReqwestClient::new();
        let token_url = "https://id.twitch.tv/oauth2/token";
        let redirect_uri = "http://localhost:9876/callback";
        let params = [
            ("client_id",     self.client_id.clone()),
            ("client_secret", self.client_secret.clone().unwrap_or_default()),
            ("code",          code),
            ("grant_type",    "authorization_code".to_string()),
            ("redirect_uri",  redirect_uri.to_string()),
        ];

        let resp = http
            .post(token_url)
            .form(&params)
            .send()
            .await
            .map_err(|e| Error::Auth(format!("HTTP error exchanging code: {e}")))?
            .error_for_status()
            .map_err(|e| Error::Auth(format!("Twitch token endpoint error: {e}")))?
            .json::<TwitchTokenResponse>()
            .await
            .map_err(|e| Error::Auth(format!("Parse error on token JSON: {e}")))?;

        let now = Utc::now();
        let expires_at = Some(now + chrono::Duration::seconds(resp.expires_in as i64));

        // For IRC usage, Twitch expects the PASS line like `PASS oauth:abcdefg`
        let with_oauth_prefix = format!("oauth:{}", resp.access_token);

        let credential = PlatformCredential {
            credential_id: Uuid::new_v4(),
            platform: Platform::TwitchIRC,
            credential_type: CredentialType::OAuth2,
            user_id: Uuid::new_v4(),
            primary_token: with_oauth_prefix,
            refresh_token: resp.refresh_token,
            additional_data: None,
            expires_at,
            created_at: now,
            updated_at: now,
            is_bot: self.is_bot,
        };

        self.pending_state = None;
        Ok(credential)
    }

    async fn refresh(&mut self, credential: &PlatformCredential) -> Result<PlatformCredential, Error> {
        // Same logic as normal Twitch refresh. Just keep the "oauth:" prefix when storing.
        let refresh_t = match &credential.refresh_token {
            Some(r) => r.clone(),
            None => return Err(Error::Auth("No refresh token for this IRC credential".into())),
        };

        let http = ReqwestClient::new();
        let token_url = "https://id.twitch.tv/oauth2/token";
        let params = [
            ("client_id", self.client_id.clone()),
            ("client_secret", self.client_secret.clone().unwrap_or_default()),
            ("refresh_token", refresh_t),
            ("grant_type", "refresh_token".to_string()),
        ];

        let resp = http
            .post(token_url)
            .form(&params)
            .send()
            .await
            .map_err(|e| Error::Auth(format!("HTTP error refreshing IRC token: {e}")))?
            .error_for_status()
            .map_err(|e| Error::Auth(format!("Twitch token endpoint error: {e}")))?
            .json::<TwitchTokenResponse>()
            .await
            .map_err(|e| Error::Auth(format!("Parse error on token JSON: {e}")))?;

        let now = Utc::now();
        let expires_at = Some(now + Duration::seconds(resp.expires_in as i64));
        let updated_primary = format!("oauth:{}", resp.access_token);

        let updated = PlatformCredential {
            credential_id: credential.credential_id,
            platform: credential.platform.clone(),
            credential_type: credential.credential_type.clone(),
            user_id: credential.user_id,
            primary_token: updated_primary,
            refresh_token: resp.refresh_token,
            additional_data: credential.additional_data.clone(),
            expires_at,
            created_at: credential.created_at,
            updated_at: now,
            is_bot: credential.is_bot,
        };
        Ok(updated)
    }

    async fn validate(&self, credential: &PlatformCredential) -> Result<bool, Error> {
        // Easiest is to do a Helix-like validate call with the token (sans "oauth:" prefix).
        let token_str = credential.primary_token
            .strip_prefix("oauth:")
            .unwrap_or(&credential.primary_token);

        let http = ReqwestClient::new();
        let resp = http
            .get("https://id.twitch.tv/oauth2/validate")
            .header("Authorization", format!("OAuth {}", token_str))
            .send()
            .await
            .map_err(|e| Error::Auth(format!("Error calling /validate: {e}")))?;

        Ok(resp.status().is_success())
    }

    async fn revoke(&mut self, credential: &PlatformCredential) -> Result<(), Error> {
        // same as Helix revoke
        let token_str = credential.primary_token
            .strip_prefix("oauth:")
            .unwrap_or(&credential.primary_token);

        let http = ReqwestClient::new();
        let revoke_url = "https://id.twitch.tv/oauth2/revoke";
        let params = [
            ("client_id", self.client_id.clone()),
            ("token", token_str.to_string()),
        ];

        let resp = http
            .post(revoke_url)
            .form(&params)
            .send()
            .await
            .map_err(|e| Error::Auth(format!("Error calling /revoke: {e}")))?;

        if resp.status().is_success() {
            Ok(())
        } else {
            error!("Failed to revoke IRC token, response code: {}", resp.status());
            Err(Error::Auth(format!("Failed to revoke token: status={}", resp.status())))
        }
    }

    fn set_is_bot(&mut self, val: bool) {
        self.is_bot = val;
    }
}