use async_trait::async_trait;
use chrono::{Utc};
use reqwest::Client as ReqwestClient;
use serde::Deserialize;
use std::sync::atomic::{AtomicUsize, Ordering};
use tracing::{error, debug};
use uuid::Uuid;

use twitch_oauth2::{
    AccessToken, ClientId,
};

use crate::Error;
use crate::auth::{AuthenticationPrompt, AuthenticationResponse, PlatformAuthenticator};
use crate::models::{CredentialType, Platform, PlatformCredential};

#[derive(Deserialize)]
struct TwitchTokenResponse {
    access_token: String,
    refresh_token: Option<String>,
    expires_in: u64,
    scope: Option<Vec<String>>,
    token_type: String,
}

/// For /validate
#[derive(Deserialize)]
struct TwitchValidateResponse {
    client_id: String,
    login: String,
    user_id: String,
    expires_in: u64,
}

static STATE_COUNTER: AtomicUsize = AtomicUsize::new(0);

pub struct TwitchAuthenticator {
    pub client_id: String,
    pub client_secret: Option<String>,
    pub is_bot: bool,
    pending_state: Option<String>,
}

impl TwitchAuthenticator {
    pub fn new(client_id: String, client_secret: Option<String>) -> Self {
        Self {
            client_id,
            client_secret,
            is_bot: false,
            pending_state: None,
        }
    }

    /// Include all scopes needed for your Helix-based flows **plus** what’s required for EventSub.
    fn build_auth_url(&self, state: &str) -> String {
        // Example set of scopes for channel bits, ads, etc. Adjust as needed.
        let scopes = vec![
            "bits:read",
            "channel:read:ads",
            "user:read:chat",
            "channel:read:subscriptions",
            "channel:moderate",
            "moderator:read:unban_requests",
            "channel:read:hype_train",
            "moderator:read:shoutouts",
        ];
        let scope_str = scopes.join(" ");
        let redirect_uri = "http://localhost:9876/callback";

        format!(
            "https://id.twitch.tv/oauth2/authorize?response_type=code&client_id={}&redirect_uri={}&scope={}&state={}",
            urlencoding::encode(&self.client_id),
            urlencoding::encode(redirect_uri),
            urlencoding::encode(&scope_str),
            urlencoding::encode(state),
        )
    }

    async fn fetch_user_login_and_id(&self, access_token: &str) -> Result<(String, String, String), Error> {
        let http_client = ReqwestClient::new();
        let response = http_client
            .get("https://id.twitch.tv/oauth2/validate")
            .header("Authorization", format!("OAuth {}", access_token))
            .send()
            .await
            .map_err(|e| Error::Auth(format!("Error calling /validate: {e}")))?;

        if !response.status().is_success() {
            return Err(Error::Auth(format!("Failed to validate token: HTTP {}", response.status())));
        }

        let validate: TwitchValidateResponse = response
            .json()
            .await
            .map_err(|e| Error::Auth(format!("Error parsing /validate response: {e}")))?;

        debug!(
            "TwitchAuthenticator /validate returned login={} user_id={}",
            validate.login, validate.user_id
        );
        // Return (login, user_id, client_id from validate)
        Ok((validate.login, validate.user_id, validate.client_id))
    }
}

#[async_trait]
impl PlatformAuthenticator for TwitchAuthenticator {
    async fn initialize(&mut self) -> Result<(), Error> {
        Ok(())
    }

    async fn start_authentication(&mut self) -> Result<AuthenticationPrompt, Error> {
        let c = STATE_COUNTER.fetch_add(1, Ordering::SeqCst);
        let state = format!("tw-state-{}", c);
        let auth_url = self.build_auth_url(&state);
        self.pending_state = Some(state);
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
        let expires_at = Some(Utc::now() + chrono::Duration::seconds(resp.expires_in as i64));

        // Fetch login, user_id, and validated client_id
        let (login, external_user_id, validate_cid) =
            self.fetch_user_login_and_id(&resp.access_token).await?;

        // Build final credential
        let credential = PlatformCredential {
            credential_id: Uuid::new_v4(),
            platform: Platform::Twitch,
            credential_type: CredentialType::OAuth2,
            user_id: Uuid::new_v4(), // Will be updated later
            primary_token: resp.access_token,
            refresh_token: resp.refresh_token,
            additional_data: Some(serde_json::json!({
                "scope": resp.scope.unwrap_or_default(),
                // KEY CHANGE: Store client_id so that it matches the token
                "client_id": self.client_id,
                // If you prefer to store the validated one:
                "validate_client_id": validate_cid,
            })),
            expires_at,
            created_at: now,
            updated_at: now,
            is_bot: self.is_bot,

            // external user/broadcaster
            platform_id: Some(external_user_id),
            user_name: login,
        };

        self.pending_state = None;
        Ok(credential)
    }

    async fn refresh(&mut self, credential: &PlatformCredential) -> Result<PlatformCredential, Error> {
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
            .map_err(|e| Error::Auth(format!("Parse error on token JSON: {e}")))?;

        let now = Utc::now();
        let expires_at = Some(now + chrono::Duration::seconds(resp.expires_in as i64));

        let (login, external_user_id, validate_cid) =
            self.fetch_user_login_and_id(&resp.access_token).await?;

        let updated = PlatformCredential {
            credential_id: credential.credential_id,
            platform: credential.platform.clone(),
            credential_type: credential.credential_type.clone(),
            user_id: credential.user_id,
            primary_token: resp.access_token,
            refresh_token: resp.refresh_token,
            additional_data: Some(serde_json::json!({
                "scope": resp.scope.unwrap_or_default(),
                "client_id": self.client_id,
                "validate_client_id": validate_cid,
            })),
            expires_at,
            created_at: credential.created_at,
            updated_at: now,
            is_bot: credential.is_bot,
            platform_id: Some(external_user_id),
            user_name: login,
        };
        Ok(updated)
    }

    async fn validate(&self, credential: &PlatformCredential) -> Result<bool, Error> {
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
        let http_client = ReqwestClient::new();
        let access_token = AccessToken::new(credential.primary_token.clone());
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