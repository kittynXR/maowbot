use async_trait::async_trait;
use chrono::{Utc, Duration};
use reqwest::Client as ReqwestClient;
use serde::Deserialize;
use std::sync::atomic::{AtomicUsize, Ordering};
use tracing::{error, debug};
use uuid::Uuid;

use maowbot_common::traits::auth_traits::{AuthenticationPrompt, AuthenticationResponse, PlatformAuthenticator};
use maowbot_common::models::platform::{Platform, PlatformCredential};
use maowbot_common::models::credential::CredentialType;
use crate::Error;

#[derive(Deserialize)]
struct TwitchTokenResponse {
    access_token: String,
    refresh_token: Option<String>,
    expires_in: u64,
    scope: Option<Vec<String>>,
    token_type: String,
}

/// Additional shape for /validate response
#[derive(Deserialize)]
struct TwitchValidateResponse {
    client_id: String,
    login: String,
    user_id: String,
    expires_in: u64,
}

/// A simple static for unique `state` each time.
static IRC_STATE_COUNTER: AtomicUsize = AtomicUsize::new(0);

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

    fn build_auth_url(&self, state: &str) -> String {
        let scopes = vec!["chat:read", "chat:edit"];
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

    /// Helper to call /validate with the raw access_token (no 'oauth:' prefix).
    async fn fetch_user_login_and_id(&self, raw_access_token: &str) -> Result<(String, String), Error> {
        let http = ReqwestClient::new();
        let resp = http
            .get("https://id.twitch.tv/oauth2/validate")
            .header("Authorization", format!("OAuth {}", raw_access_token))
            .send()
            .await
            .map_err(|e| Error::Auth(format!("Error calling /validate: {e}")))?;

        if !resp.status().is_success() {
            return Err(Error::Auth(format!(
                "Twitch /validate returned error: {}",
                resp.status()
            )));
        }

        let val = resp
            .json::<TwitchValidateResponse>()
            .await
            .map_err(|e| Error::Auth(format!("Error parsing /validate JSON: {e}")))?;

        debug!("TwitchIrcAuthenticator /validate => login={}, user_id={}", val.login, val.user_id);
        Ok((val.login, val.user_id))
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
        let auth_url = self.build_auth_url(&state);
        self.pending_state = Some(state);
        Ok(AuthenticationPrompt::Browser { url: auth_url })
    }

    async fn complete_authentication(&mut self, response: AuthenticationResponse) -> Result<PlatformCredential, Error> {
        let code = match response {
            AuthenticationResponse::Code(c) => c,
            _ => return Err(Error::Auth("Expected code in IRC flow".into())),
        };
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

        // The raw token is used with "oauth:XXXX" for PASS. But let's also fetch user login + ID
        let raw_token = resp.access_token.clone();
        let (login, user_id) = self.fetch_user_login_and_id(&raw_token).await?;

        let with_oauth_prefix = format!("oauth:{}", raw_token);
        let credential = PlatformCredential {
            credential_id: Uuid::new_v4(),
            platform: Platform::TwitchIRC,
            platform_id: Some(user_id),
            credential_type: CredentialType::OAuth2,
            user_id: Uuid::new_v4(),   // will be overwritten later if we do "complete_auth_flow_for_user"
            user_name: login,          // from /validate call
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
        // same approach as before
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
        let raw_new_access = resp.access_token.clone();

        // fetch the current login + user_id again
        let (login, user_id) = self.fetch_user_login_and_id(&raw_new_access).await?;

        let updated_primary = format!("oauth:{}", raw_new_access);

        let updated = PlatformCredential {
            credential_id: credential.credential_id,
            platform: credential.platform.clone(),
            platform_id: Some(user_id),
            credential_type: credential.credential_type.clone(),
            user_id: credential.user_id,
            user_name: login,
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
        // remove the "oauth:" prefix:
        let raw_token = credential
            .primary_token
            .trim_start_matches("oauth:")
            .to_string();

        let http = ReqwestClient::new();
        let resp = http
            .get("https://id.twitch.tv/oauth2/validate")
            .header("Authorization", format!("OAuth {}", raw_token))
            .send()
            .await
            .map_err(|e| Error::Auth(format!("Error calling /validate: {e}")))?;

        Ok(resp.status().is_success())
    }

    async fn revoke(&mut self, credential: &PlatformCredential) -> Result<(), Error> {
        let raw_token = credential
            .primary_token
            .trim_start_matches("oauth:")
            .to_string();

        let http = ReqwestClient::new();
        let revoke_url = "https://id.twitch.tv/oauth2/revoke";
        let params = [
            ("client_id", self.client_id.clone()),
            ("token", raw_token),
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