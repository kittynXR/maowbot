//! Implements Helix channel points requests, such as:
//!  - createCustomReward
//!  - deleteCustomReward
//!  - getCustomReward
//!  - getCustomRewardRedemption
//!  - updateCustomReward
//!  - updateRedemptionStatus

use serde::{Deserialize, Serialize};
use tracing::{warn, debug, trace};
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
    ///
    /// NOTE: we now do only *one* request/response cycle and parse it. The old code
    /// had a second request that caused confusion if the first had already succeeded.
    pub async fn create_custom_reward(
        &self,
        broadcaster_id: &str,
        params: &CustomRewardBody,
    ) -> Result<CustomReward, Error> {
        let url = format!(
            "https://api.twitch.tv/helix/channel_points/custom_rewards?broadcaster_id={}",
            broadcaster_id
        );

        debug!("create_custom_reward => URL='{}' body={:?}", url, params);

        let resp = self
            .http_client()
            .post(&url)
            .header("Client-Id", self.client_id())
            .header("Authorization", format!("Bearer {}", self.bearer_token()))
            .json(&params)
            .send()
            .await
            .map_err(|e| Error::Platform(format!("create_custom_reward network error: {e}")))?;

        let status_code = resp.status();
        let resp_body = resp
            .text()
            .await
            .map_err(|e| Error::Platform(format!("create_custom_reward read body error: {e}")))?;

        trace!("create_custom_reward => HTTP {} => body={}", status_code, resp_body);

        if !status_code.is_success() {
            warn!("create_custom_reward => status={} body={}", status_code, resp_body);
            return Err(Error::Platform(format!(
                "create_custom_reward: HTTP {} => {}",
                status_code, resp_body
            )));
        }

        let parsed: CustomRewardResponse = serde_json::from_str(&resp_body)
            .map_err(|e| Error::Platform(format!("create_custom_reward parse error: {e}")))?;

        if let Some(first) = parsed.data.into_iter().next() {
            debug!(
                "create_custom_reward => success => returned ID='{}' title='{}'",
                first.id, first.title
            );
            Ok(first)
        } else {
            Err(Error::Platform(
                "No reward returned by create_custom_reward".into(),
            ))
        }
    }

    /// Deletes a custom reward by ID.
    /// Required scope: `channel:manage:redemptions`
    pub async fn delete_custom_reward(
        &self,
        broadcaster_id: &str,
        reward_id: &str,
    ) -> Result<(), Error> {
        let url = format!(
            "https://api.twitch.tv/helix/channel_points/custom_rewards?broadcaster_id={}&id={}",
            broadcaster_id, reward_id
        );

        debug!("delete_custom_reward => URL='{}' reward_id='{}'", url, reward_id);

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

        debug!("delete_custom_reward => success => reward_id='{}'", reward_id);
        Ok(())
    }

    /// Gets a list of custom rewards. If `reward_ids` is provided, returns only those.
    /// If `only_manageable_rewards = true`, Twitch only returns those rewards that the token
    /// can manage.
    /// Required scope: `channel:read:redemptions` or `channel:manage:redemptions`.
    pub async fn get_custom_rewards(
        &self,
        broadcaster_id: &str,
        reward_ids: Option<&[&str]>,
        only_manageable_rewards: bool,
    ) -> Result<Vec<CustomReward>, Error> {
        let base_url = format!(
            "https://api.twitch.tv/helix/channel_points/custom_rewards?broadcaster_id={}",
            broadcaster_id
        );

        let mut url_with_params = base_url;
        if let Some(ids) = reward_ids {
            for rid in ids {
                url_with_params.push_str(&format!("&id={}", rid));
            }
        }
        if only_manageable_rewards {
            url_with_params.push_str("&only_manageable_rewards=true");
        }

        debug!("get_custom_rewards => URL='{}'", url_with_params);

        let resp = self
            .http_client()
            .get(&url_with_params)
            .header("Client-Id", self.client_id())
            .header("Authorization", format!("Bearer {}", self.bearer_token()))
            .send()
            .await
            .map_err(|e| Error::Platform(format!("get_custom_rewards network error: {e}")))?;

        let status_code = resp.status();
        let resp_text = resp.text().await.unwrap_or_default();
        trace!(
            "get_custom_rewards => HTTP {} => body length={}",
            status_code,
            resp_text.len()
        );

        if !status_code.is_success() {
            warn!("get_custom_rewards => status={} body={}", status_code, resp_text);
            return Err(Error::Platform(format!(
                "get_custom_rewards: HTTP {} => {}",
                status_code, resp_text
            )));
        }

        let parsed: CustomRewardResponse = serde_json::from_str(&resp_text)
            .map_err(|e| Error::Platform(format!("get_custom_rewards parse error: {e}")))?;

        debug!(
            "get_custom_rewards => returned {} rewards for broadcaster_id='{}'",
            parsed.data.len(),
            broadcaster_id
        );

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
        status: Option<&str>,
    ) -> Result<Vec<Redemption>, Error> {
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

        debug!(
            "get_custom_reward_redemptions => URL='{}' reward_id='{}' status={:?}",
            url_with_params, reward_id, status
        );

        let resp = self
            .http_client()
            .get(&url_with_params)
            .header("Client-Id", self.client_id())
            .header("Authorization", format!("Bearer {}", self.bearer_token()))
            .send()
            .await
            .map_err(|e| Error::Platform(format!("get_custom_reward_redemptions network error: {e}")))?;

        let status_code = resp.status();
        let resp_text = resp.text().await.unwrap_or_default();
        trace!(
            "get_custom_reward_redemptions => HTTP {} => body length={}",
            status_code,
            resp_text.len()
        );

        if !status_code.is_success() {
            warn!(
                "get_custom_reward_redemptions => status={} body={}",
                status_code, resp_text
            );
            return Err(Error::Platform(format!(
                "get_custom_reward_redemptions: HTTP {} => {}",
                status_code, resp_text
            )));
        }

        let parsed: RedemptionResponse = serde_json::from_str(&resp_text)
            .map_err(|e| Error::Platform(format!("get_custom_reward_redemptions parse error: {e}")))?;

        debug!(
            "get_custom_reward_redemptions => returned {} redemptions",
            parsed.data.len()
        );
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
        let url = format!(
            "https://api.twitch.tv/helix/channel_points/custom_rewards?broadcaster_id={}&id={}",
            broadcaster_id, reward_id
        );

        debug!("update_custom_reward => URL='{}' body={:?}", url, body);

        let resp = self
            .http_client()
            .patch(&url)
            .header("Client-Id", self.client_id())
            .header("Authorization", format!("Bearer {}", self.bearer_token()))
            .json(body)
            .send()
            .await
            .map_err(|e| Error::Platform(format!("update_custom_reward network error: {e}")))?;

        let status_code = resp.status();
        let resp_text = resp.text().await.unwrap_or_default();
        trace!("update_custom_reward => HTTP {} => body len={}", status_code, resp_text.len());

        if !status_code.is_success() {
            warn!("update_custom_reward => status={} body={}", status_code, resp_text);
            return Err(Error::Platform(format!(
                "update_custom_reward: HTTP {} => {}",
                status_code, resp_text
            )));
        }

        let mut parsed: CustomRewardResponse = serde_json::from_str(&resp_text)
            .map_err(|e| Error::Platform(format!("update_custom_reward parse error: {e}")))?;

        if let Some(first) = parsed.data.pop() {
            debug!(
                "update_custom_reward => success => ID='{}' title='{}'",
                first.id, first.title
            );
            Ok(first)
        } else {
            Err(Error::Platform(
                "No reward returned by update_custom_reward".into(),
            ))
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
        status: &str,
    ) -> Result<Vec<Redemption>, Error> {
        let base_url = format!(
            "https://api.twitch.tv/helix/channel_points/custom_rewards/redemptions\
            ?broadcaster_id={}&reward_id={}",
            broadcaster_id, reward_id
        );

        let mut url_with_params = base_url;
        for rid in redemption_ids {
            url_with_params.push_str(&format!("&id={}", rid));
        }

        debug!(
            "update_redemption_status => URL='{}' redemption_ids={:?} new_status='{}'",
            url_with_params, redemption_ids, status
        );

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

        let status_code = resp.status();
        let resp_text = resp.text().await.unwrap_or_default();
        trace!("update_redemption_status => HTTP {} => body len={}", status_code, resp_text.len());

        if !status_code.is_success() {
            warn!("update_redemption_status => status={} body={}", status_code, resp_text);
            return Err(Error::Platform(format!(
                "update_redemption_status: HTTP {} => {}",
                status_code, resp_text
            )));
        }

        let parsed: RedemptionResponse = serde_json::from_str(&resp_text)
            .map_err(|e| Error::Platform(format!("update_redemption_status parse error: {e}")))?;

        debug!(
            "update_redemption_status => returned {} updated redemptions",
            parsed.data.len()
        );
        Ok(parsed.data)
    }
}
