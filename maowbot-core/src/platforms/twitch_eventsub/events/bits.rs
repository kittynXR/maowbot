// File: maowbot-core/src/platforms/twitch_eventsub/events/bits.rs

use chrono::Utc;
use serde::Deserialize;

/// "channel.bits.use" event
#[derive(Debug, Clone, Deserialize)]
pub struct ChannelBitsUse {
    pub user_id: String,
    pub user_login: String,
    pub user_name: String,
    pub broadcaster_user_id: String,
    pub broadcaster_user_login: String,
    pub broadcaster_user_name: String,
    pub bits: u64,
    #[serde(rename = "type")]
    pub usage_type: String, // e.g. "cheer", "power_up"
    pub power_up: Option<serde_json::Value>,
    pub message: BitsMessage,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BitsMessage {
    pub text: String,
    pub fragments: Vec<BitsFragment>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BitsFragment {
    #[serde(rename = "type")]
    pub fragment_type: String,
    pub text: String,
    #[serde(default)]
    pub cheermote: Option<serde_json::Value>,
    #[serde(default)]
    pub emote: Option<serde_json::Value>,
}

/// "channel.cheer" event
#[derive(Debug, Clone, Deserialize)]
pub struct ChannelCheer {
    pub is_anonymous: bool,
    pub user_id: Option<String>,
    pub user_login: Option<String>,
    pub user_name: Option<String>,
    pub broadcaster_user_id: String,
    pub broadcaster_user_login: String,
    pub broadcaster_user_name: String,
    pub message: String,
    pub bits: u64,
}
