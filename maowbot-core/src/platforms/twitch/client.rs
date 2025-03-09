// File: maowbot-core/src/platforms/twitch/client.rs

use std::sync::Arc;
use reqwest::Client as ReqwestClient;
use serde::Deserialize;
use tracing::{warn};
use crate::Error;

/// A small wrapper client for calling various Helix endpoints.
///
/// Note that we moved the follow-related fetch call to `requests::follow`.
/// We keep this struct as a general reusable "entry point" for all Helix calls.
pub struct TwitchHelixClient {
    http: Arc<ReqwestClient>,
    bearer_token: String,
    client_id: String,
}


#[derive(Debug, Deserialize)]
pub struct ValidateTokenResponse {
    pub client_id: String,
    pub login: String,
    pub user_id: String,
    pub expires_in: u64,
}

impl TwitchHelixClient {
    /// Create a new `TwitchHelixClient`.
    ///
    /// - `bearer_token`: an OAuth token with the necessary scopes
    /// - `client_id`: from the stored credential’s `additional_data.client_id` or validated client ID
    pub fn new(bearer_token: &str, client_id: &str) -> Self {
        Self {
            http: Arc::new(ReqwestClient::new()),
            bearer_token: bearer_token.to_string(),
            client_id: client_id.to_string(),
        }
    }

    /// Expose the raw `bearer_token` if needed for special queries.
    pub fn bearer_token(&self) -> &str {
        &self.bearer_token
    }

    /// Expose the client_id for Helix requests that require it.
    pub fn client_id(&self) -> &str {
        &self.client_id
    }

    /// Returns an `Arc<ReqwestClient>` reference for advanced usage.
    pub fn http_client(&self) -> Arc<ReqwestClient> {
        self.http.clone()
    }

    pub async fn validate_token(&self) -> Result<Option<ValidateTokenResponse>, Error> {
        let url = "https://id.twitch.tv/oauth2/validate";

        let resp = self.http_client()
            .get(url)
            // Twitch says: "Authorization: OAuth <token>"
            .header("Authorization", format!("OAuth {}", self.bearer_token()))
            .send()
            .await
            .map_err(|e| Error::Platform(format!("validate_token network error: {e}")))?;

        if !resp.status().is_success() {
            // e.g. 401 => token invalid
            return Ok(None);
        }

        let parsed: ValidateTokenResponse = resp
            .json()
            .await
            .map_err(|e| Error::Platform(format!("validate_token parse error: {e}")))?;

        Ok(Some(parsed))
    }
}