// File: maowbot-core/src/platforms/twitch/requests/channel_points.rs

//! Implements Helix channel points requests, such as:
//!  - createCustomReward
//!  - deleteCustomReward
//!  - getCustomReward
//!  - getCustomRewardRedemption
//!  - updateCustomReward
//!  - updateRedemptionStatus

use serde::{Deserialize, Serialize};
use tracing::warn;

use crate::Error;
use crate::platforms::twitch::client::TwitchHelixClient;

/// Represents a single custom reward returned by Helix.
#[derive(Debug, Deserialize)]
pub struct CustomReward {
    pub broadcaster_name: Option<String>,
    pub broadcaster_login: Option<String>,
    pub broadcaster_id: String,
    pub id: String,
    pub title: String,
    pub prompt: String,
    pub cost: u64,
    pub image: Option<CustomRewardImage>,
    pub default_image: Option<CustomRewardImage>,
    pub background_color: String,
    pub is_enabled: bool,
    pub is_user_input_required: bool,
    pub max_per_stream_setting: MaxPerStreamSetting,
    pub max_per_user_per_stream_setting: MaxPerUserPerStreamSetting,
    pub global_cooldown_setting: GlobalCooldownSetting,
    pub is_paused: bool,
    pub is_in_stock: bool,
    pub should_redemptions_skip_request_queue: bool,
    pub redemptions_redeemed_current_stream: Option<u64>,
    pub cooldown_expires_at: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CustomRewardImage {
    pub url_1x: String,
    pub url_2x: String,
    pub url_4x: String,
}

#[derive(Debug, Deserialize)]
pub struct MaxPerStreamSetting {
    pub is_enabled: bool,
    pub max_per_stream: u64,
}

#[derive(Debug, Deserialize)]
pub struct MaxPerUserPerStreamSetting {
    pub is_enabled: bool,
    pub max_per_user_per_stream: u64,
}

#[derive(Debug, Deserialize)]
pub struct GlobalCooldownSetting {
    pub is_enabled: bool,
    pub global_cooldown_seconds: u64,
}

#[derive(Debug, Deserialize)]
pub struct CustomRewardResponse {
    pub data: Vec<CustomReward>,
}

/// Represents the request body for creating/updating a custom reward.
/// Typically you only include the fields you need. For "create", `title` and `cost` are required.
#[derive(Debug, Serialize, Default)]
pub struct CustomRewardBody {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost: Option<u64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_enabled: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub background_color: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_user_input_required: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_max_per_stream_enabled: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_per_stream: Option<u64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_max_per_user_per_stream_enabled: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_per_user_per_stream: Option<u64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_global_cooldown_enabled: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub global_cooldown_seconds: Option<u64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub should_redemptions_skip_request_queue: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_paused: Option<bool>,
}

/// Redemption object returned by "Get Custom Reward Redemption" calls or updates.
#[derive(Debug, Deserialize)]
pub struct Redemption {
    pub broadcaster_id: String,
    pub broadcaster_login: Option<String>,
    pub broadcaster_name: Option<String>,
    pub id: String,
    pub user_id: String,
    pub user_name: Option<String>,
    pub user_login: Option<String>,
    pub user_input: String,
    pub status: String,
    pub redeemed_at: String,
    pub reward: RedemptionReward,
}

#[derive(Debug, Deserialize)]
pub struct RedemptionReward {
    pub id: String,
    pub title: String,
    pub prompt: String,
    pub cost: u64,
}

/// Response type for redemption queries
#[derive(Debug, Deserialize)]
pub struct RedemptionResponse {
    pub data: Vec<Redemption>,
}

/// The body used when updating redemption status, e.g. FULFILLED or CANCELED.
#[derive(Debug, Serialize)]
pub struct UpdateRedemptionStatusBody {
    pub status: String,
}

impl TwitchHelixClient {
    /// Creates a custom reward in the broadcaster’s channel.
    /// Required scope: `channel:manage:redemptions`
    pub async fn create_custom_reward(
        &self,
        broadcaster_id: &str,
        params: &CustomRewardBody,
    ) -> Result<CustomReward, Error> {
        // POST https://api.twitch.tv/helix/channel_points/custom_rewards?broadcaster_id=<>
        let url = format!(
            "https://api.twitch.tv/helix/channel_points/custom_rewards?broadcaster_id={}",
            broadcaster_id
        );

        let resp = self
            .http_client()
            .post(&url)
            .header("Client-Id", self.client_id())
            .header("Authorization", format!("Bearer {}", self.bearer_token()))
            .json(&params)
            .send()
            .await
            .map_err(|e| Error::Platform(format!("create_custom_reward network error: {e}")))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body_text = resp.text().await.unwrap_or_default();
            warn!("create_custom_reward => status={} body={}", status, body_text);
            return Err(Error::Platform(format!(
                "create_custom_reward: HTTP {} => {}",
                status, body_text
            )));
        }

        let mut parsed: CustomRewardResponse = resp
            .json()
            .await
            .map_err(|e| Error::Platform(format!("create_custom_reward parse error: {e}")))?;

        // The docs say it returns a single reward, so we grab the first item
        if let Some(first) = parsed.data.pop() {
            Ok(first)
        } else {
            Err(Error::Platform("No reward returned by create_custom_reward".into()))
        }
    }

    /// Deletes a custom reward by ID.
    /// Required scope: `channel:manage:redemptions`
    pub async fn delete_custom_reward(
        &self,
        broadcaster_id: &str,
        reward_id: &str,
    ) -> Result<(), Error> {
        // DELETE https://api.twitch.tv/helix/channel_points/custom_rewards?broadcaster_id=<>&id=<>
        let url = format!(
            "https://api.twitch.tv/helix/channel_points/custom_rewards?broadcaster_id={}&id={}",
            broadcaster_id, reward_id
        );

        let resp = self
            .http_client()
            .delete(&url)
            .header("Client-Id", self.client_id())
            .header("Authorization", format!("Bearer {}", self.bearer_token()))
            .send()
            .await
            .map_err(|e| Error::Platform(format!("delete_custom_reward network error: {e}")))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body_text = resp.text().await.unwrap_or_default();
            warn!("delete_custom_reward => status={} body={}", status, body_text);
            return Err(Error::Platform(format!(
                "delete_custom_reward: HTTP {} => {}",
                status, body_text
            )));
        }

        // If successful, there's no body; status is 204 No Content
        Ok(())
    }

    /// Gets a list of custom rewards. If `reward_ids` is provided, returns only those.
    /// Required scope: `channel:read:redemptions` or `channel:manage:redemptions`.
    pub async fn get_custom_rewards(
        &self,
        broadcaster_id: &str,
        reward_ids: Option<&[&str]>,
        only_manageable_rewards: bool,
    ) -> Result<Vec<CustomReward>, Error> {
        // GET https://api.twitch.tv/helix/channel_points/custom_rewards?broadcaster_id=<>&id=...&only_manageable_rewards=true/false
        let base_url = format!(
            "https://api.twitch.tv/helix/channel_points/custom_rewards?broadcaster_id={}",
            broadcaster_id
        );

        // Append reward_id params if present
        let mut url_with_params = base_url;
        if let Some(ids) = reward_ids {
            for rid in ids {
                url_with_params.push_str(&format!("&id={}", rid));
            }
        }

        if only_manageable_rewards {
            url_with_params.push_str("&only_manageable_rewards=true");
        }

        let resp = self
            .http_client()
            .get(&url_with_params)
            .header("Client-Id", self.client_id())
            .header("Authorization", format!("Bearer {}", self.bearer_token()))
            .send()
            .await
            .map_err(|e| Error::Platform(format!("get_custom_rewards network error: {e}")))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body_text = resp.text().await.unwrap_or_default();
            warn!("get_custom_rewards => status={} body={}", status, body_text);
            return Err(Error::Platform(format!(
                "get_custom_rewards: HTTP {} => {}",
                status, body_text
            )));
        }

        let parsed: CustomRewardResponse = resp
            .json()
            .await
            .map_err(|e| Error::Platform(format!("get_custom_rewards parse error: {e}")))?;

        Ok(parsed.data)
    }

    /// Gets redemptions for a specified reward. If `redemption_ids` is provided, returns only those.
    /// Otherwise, you must specify a `status`.
    /// Required scope: `channel:read:redemptions` or `channel:manage:redemptions`.
    pub async fn get_custom_reward_redemptions(
        &self,
        broadcaster_id: &str,
        reward_id: &str,
        redemption_ids: Option<&[&str]>,
        status: Option<&str>, // e.g. Some("UNFULFILLED") or Some("FULFILLED")
    ) -> Result<Vec<Redemption>, Error> {
        // GET https://api.twitch.tv/helix/channel_points/custom_rewards/redemptions?broadcaster_id=<>&reward_id=<>&[id=...]&status=...
        let base_url = format!(
            "https://api.twitch.tv/helix/channel_points/custom_rewards/redemptions\
             ?broadcaster_id={}&reward_id={}",
            broadcaster_id, reward_id
        );

        let mut url_with_params = base_url;
        if let Some(ids) = redemption_ids {
            for rid in ids {
                url_with_params.push_str(&format!("&id={}", rid));
            }
        } else if let Some(st) = status {
            url_with_params.push_str(&format!("&status={}", st));
        } else {
            return Err(Error::Platform(
                "get_custom_reward_redemptions requires either redemption_ids or status".into(),
            ));
        }

        let resp = self
            .http_client()
            .get(&url_with_params)
            .header("Client-Id", self.client_id())
            .header("Authorization", format!("Bearer {}", self.bearer_token()))
            .send()
            .await
            .map_err(|e| Error::Platform(format!("get_custom_reward_redemptions network error: {e}")))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body_text = resp.text().await.unwrap_or_default();
            warn!("get_custom_reward_redemptions => status={} body={}", status, body_text);
            return Err(Error::Platform(format!(
                "get_custom_reward_redemptions: HTTP {} => {}",
                status, body_text
            )));
        }

        let parsed: RedemptionResponse = resp
            .json()
            .await
            .map_err(|e| Error::Platform(format!("get_custom_reward_redemptions parse error: {e}")))?;

        Ok(parsed.data)
    }

    /// Updates a custom reward by ID.
    /// Only include fields in `body` that you want to modify.
    /// Required scope: `channel:manage:redemptions`.
    pub async fn update_custom_reward(
        &self,
        broadcaster_id: &str,
        reward_id: &str,
        body: &CustomRewardBody,
    ) -> Result<CustomReward, Error> {
        // PATCH https://api.twitch.tv/helix/channel_points/custom_rewards?broadcaster_id=<>&id=<>
        let url = format!(
            "https://api.twitch.tv/helix/channel_points/custom_rewards?broadcaster_id={}&id={}",
            broadcaster_id, reward_id
        );

        let resp = self
            .http_client()
            .patch(&url)
            .header("Client-Id", self.client_id())
            .header("Authorization", format!("Bearer {}", self.bearer_token()))
            .json(body)
            .send()
            .await
            .map_err(|e| Error::Platform(format!("update_custom_reward network error: {e}")))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body_text = resp.text().await.unwrap_or_default();
            warn!("update_custom_reward => status={} body={}", status, body_text);
            return Err(Error::Platform(format!(
                "update_custom_reward: HTTP {} => {}",
                status, body_text
            )));
        }

        let mut parsed: CustomRewardResponse = resp
            .json()
            .await
            .map_err(|e| Error::Platform(format!("update_custom_reward parse error: {e}")))?;

        if let Some(first) = parsed.data.pop() {
            Ok(first)
        } else {
            Err(Error::Platform("No reward returned by update_custom_reward".into()))
        }
    }

    /// Updates one or more redemption statuses for a given reward.
    /// You can pass up to 50 redemption IDs. Status can be "FULFILLED" or "CANCELED".
    /// Required scope: `channel:manage:redemptions`.
    pub async fn update_redemption_status(
        &self,
        broadcaster_id: &str,
        reward_id: &str,
        redemption_ids: &[&str],
        status: &str, // "FULFILLED" or "CANCELED"
    ) -> Result<Vec<Redemption>, Error> {
        // PATCH https://api.twitch.tv/helix/channel_points/custom_rewards/redemptions?broadcaster_id=<>&reward_id=<>&id=...
        let base_url = format!(
            "https://api.twitch.tv/helix/channel_points/custom_rewards/redemptions\
            ?broadcaster_id={}&reward_id={}",
            broadcaster_id, reward_id
        );

        let mut url_with_params = base_url;
        for rid in redemption_ids {
            url_with_params.push_str(&format!("&id={}", rid));
        }

        let body = UpdateRedemptionStatusBody {
            status: status.to_string(),
        };

        let resp = self
            .http_client()
            .patch(&url_with_params)
            .header("Client-Id", self.client_id())
            .header("Authorization", format!("Bearer {}", self.bearer_token()))
            .json(&body)
            .send()
            .await
            .map_err(|e| Error::Platform(format!("update_redemption_status network error: {e}")))?;

        if !resp.status().is_success() {
            let http_status = resp.status();
            let body_text = resp.text().await.unwrap_or_default();
            warn!("update_redemption_status => status={} body={}", http_status, body_text);
            return Err(Error::Platform(format!(
                "update_redemption_status: HTTP {} => {}",
                http_status, body_text
            )));
        }

        let parsed: RedemptionResponse = resp
            .json()
            .await
            .map_err(|e| Error::Platform(format!("update_redemption_status parse error: {e}")))?;

        Ok(parsed.data)
    }
}
