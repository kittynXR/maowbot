use std::sync::Arc;
use chrono::{DateTime, Utc};
use reqwest::Client as ReqwestClient;
use serde::Deserialize;
use tracing::{debug, warn};

use crate::Error;

/// Response from `GET /helix/channels/followers`.
#[derive(Debug, Deserialize)]
pub struct ChannelFollowersResponse {
    pub data: Vec<FollowerData>,
    /// The total number of users that follow this broadcaster.
    pub total: u32,
    /// Pagination information. Not used here unless you’re paging through full results.
    pub pagination: Pagination,
}

#[derive(Debug, Deserialize)]
pub struct FollowerData {
    /// The UTC timestamp when the user started following the broadcaster.
    pub followed_at: String,
    /// The user’s ID (Twitch numeric user ID).
    pub user_id: String,
    /// The user’s login name (all-lowercase).
    pub user_login: String,
    /// The user’s display name.
    pub user_name: String,
}

/// Pagination object, usually includes a `cursor` field.
#[derive(Debug, Deserialize, Default)]
pub struct Pagination {
    pub cursor: Option<String>,
}

/// A small wrapper client for calling the new Helix follower endpoint.
pub struct TwitchHelixClient {
    http: Arc<ReqwestClient>,
    bearer_token: String,
    client_id: String,
}

impl TwitchHelixClient {
    /// Create a new `TwitchHelixClient`.
    ///
    /// - `bearer_token`: an OAuth token with scope `moderator:read:followers`.
    /// - `client_id`: from the stored credential’s `additional_data.client_id` or validated client ID.
    pub fn new(bearer_token: &str, client_id: &str) -> Self {
        Self {
            http: Arc::new(ReqwestClient::new()),
            bearer_token: bearer_token.to_string(),
            client_id: client_id.to_string(),
        }
    }

    /// Checks whether `viewer_id` follows `broadcaster_id`.
    ///
    /// If so, returns the `followed_at` timestamp.
    /// If not following (or the API call returns an empty set), returns `Ok(None)`.
    /// If there's a network error, invalid token, or 4xx/5xx, returns `Err(...)`.
    ///
    /// **Important**: This requires:
    /// - A user token with `moderator:read:followers` scope.
    /// - The token’s user must be `broadcaster_id` OR a moderator of `broadcaster_id`.
    /// - The `client_id` must match the app used to obtain that token.
    pub async fn fetch_follow_date(
        &self,
        viewer_id: &str,
        broadcaster_id: &str,
    ) -> Result<Option<DateTime<Utc>>, Error> {
        if viewer_id.is_empty() || broadcaster_id.is_empty() {
            warn!("TwitchHelixClient::fetch_follow_date called with empty viewer_id or broadcaster_id");
            return Ok(None);
        }

        // For one specific user, specify both broadcaster_id and user_id
        let url = format!(
            "https://api.twitch.tv/helix/channels/followers?broadcaster_id={}&user_id={}",
            broadcaster_id, viewer_id
        );

        let resp = self.http
            .get(&url)
            .header("Client-Id", &self.client_id)
            .header("Authorization", format!("Bearer {}", self.bearer_token))
            .send()
            .await
            .map_err(|e| Error::Platform(format!("Network error: {e}")))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body_text = resp.text().await.unwrap_or_default();
            warn!("TwitchHelixClient => status={} body={}", status, body_text);

            // Return a direct error: 401, 403, 400, etc.
            return Err(Error::Platform(format!(
                "Twitch API error: HTTP {} => {}",
                status, body_text
            )));
        }

        let parsed: ChannelFollowersResponse = resp
            .json()
            .await
            .map_err(|e| Error::Platform(format!("Error parsing /channels/followers JSON: {e}")))?;

        debug!(
            "fetch_follow_date => total={} data.len()={}",
            parsed.total, parsed.data.len()
        );

        // If data is empty, it means the viewer doesn't follow (or is unauthorized to see the data).
        if parsed.data.is_empty() {
            return Ok(None);
        }

        // Usually only one result if you're specifying `user_id=xxx`, but we’ll take the first.
        let first = &parsed.data[0];
        let followed_at = DateTime::parse_from_rfc3339(&first.followed_at)
            .map_err(|e| Error::Platform(format!("Failed to parse followed_at: {e}")))?
            .with_timezone(&Utc);

        Ok(Some(followed_at))
    }
}
