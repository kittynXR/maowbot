// File: maowbot-core/src/platforms/twitch_eventsub/events/channel_points.rs

use serde::Deserialize;
use chrono::{DateTime, Utc};

// ------------------------------------------------------------------------
// "channel.channel_points_automatic_reward_redemption.add" event
// ------------------------------------------------------------------------
#[derive(Debug, Clone, Deserialize)]
pub struct ChannelPointsAutomaticRewardRedemptionAddV2 {
    pub broadcaster_user_id: String,
    pub broadcaster_user_name: String,
    pub broadcaster_user_login: String,
    pub user_id: String,
    pub user_name: String,
    pub user_login: String,
    pub id: String,
    pub reward: AutomaticReward,
    pub message: AutomaticRewardMessage,
    pub redeemed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AutomaticReward {
    #[serde(rename = "type")]
    pub reward_type: String,
    pub channel_points: u64,
    #[serde(default)]
    pub emote: Option<RewardEmote>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RewardEmote {
    pub id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AutomaticRewardMessage {
    pub text: String,
    #[serde(default)]
    pub fragments: Vec<AutomaticRewardFragment>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AutomaticRewardFragment {
    #[serde(rename = "type")]
    pub fragment_type: String,
    pub text: String,
    #[serde(default)]
    pub emote: Option<RewardEmote>,
}

// ------------------------------------------------------------------------
// "channel.channel_points_custom_reward.{add|update|remove}" event
// ------------------------------------------------------------------------
#[derive(Debug, Clone, Deserialize)]
pub struct ChannelPointsCustomReward {
    pub id: String,
    pub broadcaster_user_id: String,
    pub broadcaster_user_login: String,
    pub broadcaster_user_name: String,
    pub is_enabled: bool,
    pub is_paused: bool,
    pub is_in_stock: bool,
    pub title: String,
    pub cost: u64,
    pub prompt: String,
    pub is_user_input_required: bool,
    pub should_redemptions_skip_request_queue: bool,
    #[serde(default)]
    pub cooldown_expires_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub redemptions_redeemed_current_stream: Option<u64>,
    pub max_per_stream: RewardLimit,
    pub max_per_user_per_stream: RewardLimit,
    pub global_cooldown: GlobalCooldown,
    pub background_color: String,
    #[serde(default)]
    pub image: Option<RewardImage>,
    #[serde(default)]
    pub default_image: Option<RewardImage>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RewardLimit {
    pub is_enabled: bool,
    pub value: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GlobalCooldown {
    pub is_enabled: bool,
    pub seconds: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RewardImage {
    pub url_1x: String,
    pub url_2x: String,
    pub url_4x: String,
}

// ------------------------------------------------------------------------
// "channel.channel_points_custom_reward_redemption.{add|update}" event
// ------------------------------------------------------------------------
#[derive(Debug, Clone, Deserialize)]
pub struct ChannelPointsCustomRewardRedemption {
    pub id: String,
    pub broadcaster_user_id: String,
    pub broadcaster_user_login: String,
    pub broadcaster_user_name: String,
    pub user_id: String,
    pub user_login: String,
    pub user_name: String,
    pub user_input: String,
    pub status: String, // "unfulfilled", "fulfilled", "canceled", etc.
    pub reward: RedemptionReward,
    pub redeemed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RedemptionReward {
    pub id: String,
    pub title: String,
    pub cost: u64,
    pub prompt: String,
}
