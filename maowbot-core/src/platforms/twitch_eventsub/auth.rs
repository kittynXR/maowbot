// maowbot-core/src/platforms/twitch_eventsub/auth.rs

use async_trait::async_trait;
use chrono::Utc;
use reqwest::Client as ReqwestClient;
use serde::Deserialize;
use tracing::{debug, error};

use maowbot_common::traits::auth_traits::{AuthenticationPrompt, AuthenticationResponse, PlatformAuthenticator};
use maowbot_common::models::platform::{PlatformCredential};
use crate::Error;

// We'll reuse the same Twitch token exchange JSON structure:
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

/// The TwitchEventSubAuthenticator is meant to reuse Helix logic
/// so that refreshing actually calls the Twitch OAuth token endpoint.
pub struct TwitchEventSubAuthenticator {
    pub client_id: String,
    pub client_secret: Option<String>,
    pub is_broadcaster: bool,
    pub is_teammate: bool,
    pub is_bot: bool,
}

impl TwitchEventSubAuthenticator {
    pub fn new(client_id: String, client_secret: Option<String>) -> Self {
        Self {
            client_id,
            client_secret,
            is_broadcaster: false,
            is_teammate: false,
            is_bot: false,
        }
    }

    async fn fetch_user_login_and_id(
        &self,
        access_token: &str,
    ) -> Result<(String, String, String), Error> {
        let http_client = ReqwestClient::new();
        let response = http_client
            .get("https://id.twitch.tv/oauth2/validate")
            .header("Authorization", format!("OAuth {}", access_token))
            .send()
            .await
            .map_err(|e| Error::Auth(format!("Error calling /validate: {e}")))?;

        if !response.status().is_success() {
            return Err(Error::Auth(format!(
                "Failed to validate token: HTTP {}",
                response.status()
            )));
        }

        let validate: TwitchValidateResponse = response
            .json()
            .await
            .map_err(|e| Error::Auth(format!("Error parsing /validate response: {e}")))?;

        debug!(
            "[EventSub] /validate => login={} user_id={} client_id={}",
            validate.login, validate.user_id, validate.client_id
        );

        Ok((validate.login, validate.user_id, validate.client_id))
    }
}

#[async_trait]
impl PlatformAuthenticator for TwitchEventSubAuthenticator {
    async fn initialize(&mut self) -> Result<(), Error> {
        // No special initialization needed
        Ok(())
    }

    /// Typically, for an EventSub credential, we do NOT do a new user-facing flow.
    /// In your usage, you might just "reuse" Helix tokens or do a client_credentials flow.
    /// If you do want an OAuth code flow, you can mimic "TwitchAuthenticator::start_authentication".
    async fn start_authentication(&mut self) -> Result<AuthenticationPrompt, Error> {
        // If your design reuses Helix tokens, you can just return None or a note.
        Ok(AuthenticationPrompt::None)
    }

    /// If needed, this could handle finishing an OAuth code flow. But in your usage,
    /// you often just "store" the same tokens from the Helix credential. Or do client_credentials.
    async fn complete_authentication(
        &mut self,
        _response: AuthenticationResponse
    ) -> Result<PlatformCredential, Error> {
        // For re-using Helix, you'd just store a cloned token. Or handle code flows similarly.
        Err(Error::Auth(
            "[EventSub] complete_authentication not supported here. Usually we reuse Helix tokens."
                .into(),
        ))
    }

    /// The important fix: we must actually refresh the token by calling Twitch's
    /// token endpoint, just like the Helix logic. That was missing before.
    async fn refresh(&mut self, credential: &PlatformCredential) -> Result<PlatformCredential, Error> {
        let refresh_token = match credential.refresh_token.as_ref() {
            Some(r) => r.clone(),
            None => {
                return Err(Error::Auth(
                    "[EventSub] No refresh token available.".into(),
                ))
            }
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
            .map_err(|e| Error::Auth(format!("HTTP error refreshing token (eventsub): {e}")))?
            .error_for_status()
            .map_err(|e| Error::Auth(format!("Twitch token endpoint error (eventsub): {e}")))?
            .json::<TwitchTokenResponse>()
            .await
            .map_err(|e| Error::Auth(format!("Parse error on token JSON (eventsub): {e}")))?;

        let now = Utc::now();
        let expires_at = Some(now + chrono::Duration::seconds(resp.expires_in as i64));

        let (login, external_user_id, validate_cid) =
            self.fetch_user_login_and_id(&resp.access_token).await?;

        let updated_cred = PlatformCredential {
            credential_id: credential.credential_id,
            platform: credential.platform.clone(),
            credential_type: credential.credential_type.clone(),
            user_id: credential.user_id,
            user_name: login,
            platform_id: Some(external_user_id),
            primary_token: resp.access_token,
            refresh_token: resp.refresh_token,
            additional_data: Some(serde_json::json!({
                "scope": resp.scope.unwrap_or_default(),
                "client_id": self.client_id,
                "validate_client_id": validate_cid,
                "note": "EventSub re-uses Helix"
            })),
            expires_at,
            created_at: credential.created_at,
            updated_at: now,
            is_broadcaster: credential.is_broadcaster,
            is_teammate: credential.is_teammate,
            is_bot: credential.is_bot,
        };

        Ok(updated_cred)
    }

    /// For validation, we can reuse the same /validate logic to check if it's still good.
    async fn validate(&self, credential: &PlatformCredential) -> Result<bool, Error> {
        let http_client = ReqwestClient::new();
        let test_url = "https://id.twitch.tv/oauth2/validate";
        let resp = http_client
            .get(test_url)
            .header(
                "Authorization",
                format!("OAuth {}", credential.primary_token),
            )
            .send()
            .await
            .map_err(|e| Error::Auth(format!("HTTP error calling /validate: {e}")))?;

        if resp.status().is_success() {
            Ok(true)
        } else {
            error!(
                "[EventSub] validate() => Token invalid, status={}",
                resp.status()
            );
            Ok(false)
        }
    }

    /// If you want to revoke your EventSub token, do the same approach as Helix:
    async fn revoke(&mut self, credential: &PlatformCredential) -> Result<(), Error> {
        // You can do the same AccessToken::revoke_token approach if desired,
        // or just let them expire.
        // For a minimal fix, we'll just do a small HTTP request:
        let http_client = ReqwestClient::new();
        let url = format!(
            "https://id.twitch.tv/oauth2/revoke?client_id={}&token={}",
            self.client_id, credential.primary_token
        );
        let resp = http_client
            .post(&url)
            .send()
            .await
            .map_err(|e| Error::Auth(format!("Error calling revoke for eventsub: {e}")))?;

        if resp.status().is_success() {
            Ok(())
        } else {
            Err(Error::Auth(format!(
                "EventSub revoke returned status={}",
                resp.status()
            )))
        }
    }

    fn set_is_broadcaster(&mut self, val: bool) {
        self.is_bot = val;
    }

    fn set_is_teammate(&mut self, val: bool) {
        self.is_teammate = val;
    }

    fn set_is_bot(&mut self, val: bool) {
        self.is_bot = val;
    }
}
