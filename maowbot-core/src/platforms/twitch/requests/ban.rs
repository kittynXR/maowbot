//! Helix ⟶ POST /moderation/bans
//! Supports both permanent bans (omit `duration`) and time‑outs.

use serde::{Deserialize, Serialize};
use crate::Error;
use crate::platforms::twitch::client::TwitchHelixClient;

/// JSON body sent to Helix.
#[derive(Debug, Serialize)]
struct BanRequest<'a> {
    data: BanRequestData<'a>,
}

#[derive(Debug, Serialize)]
struct BanRequestData<'a> {
    user_id:   &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    duration:  Option<u32>,          //
    #[serde(skip_serializing_if = "Option::is_none")]
    reason:    Option<&'a str>,      //
}

/// Partial response struct (we don’t need every field).
#[derive(Debug, Deserialize)]
struct BanResponse {
    data: Vec<BanResult>,
}
#[derive(Debug, Deserialize)]
struct BanResult {
    user_id: String,
    end_time: Option<String>,
}

impl TwitchHelixClient {
    /// Ban or timeout a user.
    ///
    /// * `duration`: `Some(seconds)` ⇒ timeout, `None` ⇒ permanent ban.
    pub async fn ban_user(
        &self,
        broadcaster_id: &str,
        moderator_id:   &str,
        user_id:        &str,
        duration:       Option<u32>,
        reason:         Option<&str>,
    ) -> Result<(), Error> {
        let url = format!(
            "https://api.twitch.tv/helix/moderation/bans?broadcaster_id={}&moderator_id={}",
            broadcaster_id, moderator_id
        );

        let body = BanRequest {
            data: BanRequestData { user_id, duration, reason },
        };

        let resp = self
            .http_client()
            .post(&url)
            .header("Client-Id",  self.client_id())
            .header("Authorization", format!("Bearer {}", self.bearer_token()))
            .json(&body)
            .send()
            .await
            .map_err(|e| Error::Platform(format!("ban_user network error: {e}")))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text   = resp.text().await.unwrap_or_default();
            return Err(Error::Platform(format!("ban_user: HTTP {status} ⇒ {text}")));
        }
        // (Success JSON is ignored – nothing useful required downstream.)
        Ok(())
    }

    /// Resolve login → user‑id (cheap helper for mod tools).
    pub async fn fetch_user_id(&self, login: &str) -> Result<Option<String>, Error> {
        let url = format!("https://api.twitch.tv/helix/users?login={}", login.to_lowercase());
        let resp = self
            .http_client()
            .get(&url)
            .header("Client-Id",  self.client_id())
            .header("Authorization", format!("Bearer {}", self.bearer_token()))
            .send()
            .await
            .map_err(|e| Error::Platform(format!("fetch_user_id network error: {e}")))?;

        if !resp.status().is_success() {
            return Ok(None);
        }
        #[derive(Deserialize)]
        struct Users { data: Vec<User>, }
        #[derive(Deserialize)]
        struct User  { id: String, }
        let parsed: Users = resp.json().await
            .map_err(|e| Error::Platform(format!("fetch_user_id parse error: {e}")))?;
        Ok(parsed.data.first().map(|u| u.id.clone()))
    }
}
