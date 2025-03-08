// File: maowbot-core/src/platforms/twitch/requests/follow.rs

use chrono::{DateTime, Utc};
use serde::Deserialize;
use tracing::{warn};
use crate::Error;
use crate::platforms::twitch::client::TwitchHelixClient;

/// Response from `GET /helix/channels/followers`.
#[derive(Debug, Deserialize)]
pub struct ChannelFollowersResponse {
    pub data: Vec<FollowerData>,
    /// The total number of users that follow this broadcaster.
    pub total: u32,
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

/// Pagination object, usually includes a `cursor` field for paging results.
#[derive(Debug, Deserialize, Default)]
pub struct Pagination {
    pub cursor: Option<String>,
}

impl TwitchHelixClient {
    /// Checks whether `viewer_id` follows `broadcaster_id`.
    ///
    /// If so, returns the `followed_at` timestamp. If not following, returns `Ok(None)`.
    ///
    /// Requires a user token with `moderator:read:followers` scope, where the user is either:
    ///   - the broadcaster themselves, or
    ///   - a moderator of the broadcaster’s channel.
    pub async fn fetch_follow_date(
        &self,
        viewer_id: &str,
        broadcaster_id: &str,
    ) -> Result<Option<DateTime<Utc>>, Error> {
        if viewer_id.is_empty() || broadcaster_id.is_empty() {
            warn!("fetch_follow_date called with empty viewer_id or broadcaster_id");
            return Ok(None);
        }

        // Helix endpoint:
        // GET https://api.twitch.tv/helix/channels/followers?broadcaster_id=<>&user_id=<>
        let url = format!(
            "https://api.twitch.tv/helix/channels/followers?broadcaster_id={}&user_id={}",
            broadcaster_id, viewer_id
        );

        let resp = self
            .http_client()
            .get(&url)
            .header("Client-Id", self.client_id())
            .header("Authorization", format!("Bearer {}", self.bearer_token()))
            .send()
            .await
            .map_err(|e| Error::Platform(format!("Network error: {e}")))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body_text = resp.text().await.unwrap_or_default();
            warn!("fetch_follow_date => status={} body={}", status, body_text);

            return Err(Error::Platform(format!(
                "Twitch API error: HTTP {} => {}",
                status, body_text
            )));
        }

        let parsed: ChannelFollowersResponse = resp
            .json()
            .await
            .map_err(|e| Error::Platform(format!("Error parsing /channels/followers JSON: {e}")))?;

        if parsed.data.is_empty() {
            return Ok(None);
        }

        // Usually only one record if a single viewer_id is requested
        let first = &parsed.data[0];
        let followed_at = DateTime::parse_from_rfc3339(&first.followed_at)
            .map_err(|e| Error::Platform(format!("Failed to parse followed_at: {e}")))?
            .with_timezone(&Utc);

        Ok(Some(followed_at))
    }

    // Potentially you could add more advanced "list all followers" or "check if many users follow"
    // but for now, we only replicate the old "fetch_follow_date" function and expansions can be done later.
}
