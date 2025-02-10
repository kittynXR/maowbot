use async_trait::async_trait;
use chrono::{DateTime, Utc};
use reqwest::Client as ReqwestClient;
use serde::Deserialize;
use std::sync::atomic::{AtomicUsize, Ordering};
use tracing::error;
use uuid::Uuid;

use twitch_oauth2::{
    AccessToken, ClientId, ClientSecret, RefreshToken, Scope, TwitchToken,
};

use crate::Error;
use crate::auth::{AuthenticationPrompt, AuthenticationResponse, PlatformAuthenticator};
use crate::models::{CredentialType, Platform, PlatformCredential};

/// Matches Twitch's JSON from the token endpoint
#[derive(Deserialize)]
struct TwitchTokenResponse {
    access_token: String,
    refresh_token: Option<String>,
    expires_in: u64,
    scope: Option<Vec<String>>,
    token_type: String, // e.g. "bearer"
}

/// Twitch code flow with client_secret, no PKCE, for twitch_oauth2 v0.15.1
pub struct TwitchAuthenticator {
    pub client_id: String,
    pub client_secret: Option<String>,
    pub is_bot: bool,

    /// We'll store 'state' from `start_authentication` if you want to do a state-check
    pending_state: Option<String>,
}

/// Simple static for unique state each time
static STATE_COUNTER: AtomicUsize = AtomicUsize::new(0);

impl TwitchAuthenticator {
    pub fn new(client_id: String, client_secret: Option<String>) -> Self {
        Self {
            client_id,
            client_secret,
            is_bot: false,
            pending_state: None,
        }
    }

    fn build_auth_url(&self, state: &str) -> String {
        // Example scopes
        let scopes = vec!["chat:read", "chat:edit", "channel:read:subscriptions"];
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
impl PlatformAuthenticator for TwitchAuthenticator {
    async fn initialize(&mut self) -> Result<(), Error> {
        Ok(())
    }

    async fn start_authentication(&mut self) -> Result<AuthenticationPrompt, Error> {
        // Generate a "state"
        let c = STATE_COUNTER.fetch_add(1, Ordering::SeqCst);
        let state = format!("tw-state-{}", c);

        // Build auth URL
        let auth_url = self.build_auth_url(&state);

        // Save it if you want to do state-check
        self.pending_state = Some(state);

        // Return a prompt for TUI
        Ok(AuthenticationPrompt::Browser { url: auth_url })
    }

    async fn complete_authentication(
        &mut self,
        response: AuthenticationResponse
    ) -> Result<PlatformCredential, Error> {
        let code = match response {
            AuthenticationResponse::Code(c) => c,
            _ => return Err(Error::Auth("Expected code in complete_authentication".into())),
        };

        // (Optional) If you want to do `?state=` check from the callback, do it here

        // Exchange code -> access_token
        let http_client = ReqwestClient::new();
        let token_url = "https://id.twitch.tv/oauth2/token";
        let redirect_uri = "http://localhost:9876/callback";

        let params = [
            ("client_id",     self.client_id.clone()),
            ("client_secret", self.client_secret.clone().unwrap_or_default()),
            ("code",          code),
            ("grant_type",    "authorization_code".to_string()),
            ("redirect_uri",  redirect_uri.to_string()),
        ];

        let resp = http_client
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

        let credential = PlatformCredential {
            credential_id: Uuid::new_v4(),
            platform: Platform::Twitch,
            credential_type: CredentialType::OAuth2,
            user_id: Uuid::new_v4(),
            primary_token: resp.access_token,
            refresh_token: resp.refresh_token,
            additional_data: Some(serde_json::json!({
                "scope": resp.scope.unwrap_or_default(),
            })),
            expires_at,
            created_at: now,
            updated_at: now,
            is_bot: self.is_bot,
        };

        // Clear the stored state
        self.pending_state = None;
        Ok(credential)
    }

    async fn refresh(&mut self, credential: &PlatformCredential) -> Result<PlatformCredential, Error> {
        // If we have a refresh token, do same approach
        let refresh_token = match credential.refresh_token.as_ref() {
            Some(r) => r.clone(),
            None => return Err(Error::Auth("No refresh token available.".into())),
        };

        let http_client = ReqwestClient::new();
        let token_url = "https://id.twitch.tv/oauth2/token";
        let params = [
            ("client_id", self.client_id.clone()),
            ("client_secret", self.client_secret.clone().unwrap_or_default()),
            ("refresh_token", refresh_token),
            ("grant_type", "refresh_token".to_string()),
        ];

        let resp = http_client
            .post(token_url)
            .form(&params)
            .send()
            .await
            .map_err(|e| Error::Auth(format!("HTTP error refreshing token: {e}")))?
            .error_for_status()
            .map_err(|e| Error::Auth(format!("Twitch token endpoint error: {e}")))?
            .json::<TwitchTokenResponse>()
            .await
            .map_err(|e| Error::Auth(format!("Parse error on refresh JSON: {e}")))?;

        let now = Utc::now();
        let expires_at = Some(now + chrono::Duration::seconds(resp.expires_in as i64));

        let updated = PlatformCredential {
            credential_id: credential.credential_id.clone(),
            platform: credential.platform.clone(),
            credential_type: credential.credential_type.clone(),
            user_id: credential.user_id.clone(),
            primary_token: resp.access_token,
            refresh_token: resp.refresh_token,
            additional_data: Some(serde_json::json!({
                "scope": resp.scope.unwrap_or_default(),
            })),
            expires_at,
            created_at: credential.created_at,
            updated_at: now,
            is_bot: credential.is_bot,
        };

        Ok(updated)
    }

    async fn validate(&self, credential: &PlatformCredential) -> Result<bool, Error> {
        // `validate_token(&self, client: &impl Client)` in 0.15.1 has only 1 param
        let http_client = ReqwestClient::new();
        let access_token = AccessToken::new(credential.primary_token.clone());

        match access_token.validate_token(&http_client).await {
            Ok(_valid) => Ok(true),
            Err(e) => {
                error!("Twitch validate_token error => {e}");
                Ok(false)
            }
        }
    }

    async fn revoke(&mut self, credential: &PlatformCredential) -> Result<(), Error> {
        // In v0.15.1, `revoke_token` is `fn revoke_token(self, client: &impl Client, client_id: &ClientId)`
        // It does not accept client_secret. So:
        let http_client = ReqwestClient::new();
        let access_token = AccessToken::new(credential.primary_token.clone());

        // We'll pass just &ClientId:
        let cid = ClientId::new(self.client_id.clone());

        match access_token.revoke_token(&http_client, &cid).await {
            Ok(_) => Ok(()),
            Err(e) => {
                error!("Failed to revoke => {e}");
                Err(Error::Auth(format!("Failed to revoke Twitch token: {e}")))
            }
        }
    }

    fn set_is_bot(&mut self, val: bool) {
        self.is_bot = val;
    }
}