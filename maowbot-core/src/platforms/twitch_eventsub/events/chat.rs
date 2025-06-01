// File: maowbot-core/src/platforms/twitch_eventsub/events/chat.rs

use serde::Deserialize;

/// "channel.chat.notification" event
#[derive(Debug, Clone, Deserialize)]
pub struct ChannelChatNotification {
    pub broadcaster_user_id: String,
    pub broadcaster_user_login: String,
    pub broadcaster_user_name: String,
    pub chatter_user_id: String,
    pub chatter_user_login: String,
    pub chatter_user_name: String,
    pub chatter_is_anonymous: bool,
    pub color: String,
    #[serde(default)]
    pub badges: Vec<serde_json::Value>,
    pub system_message: String,
    pub message_id: String,
    pub message: ChatMessageBody,
    #[serde(default)]
    pub notice_type: String,
    // The many conditional fields:
    #[serde(default)]
    pub sub: Option<serde_json::Value>,
    #[serde(default)]
    pub resub: Option<serde_json::Value>,
    #[serde(default)]
    pub sub_gift: Option<serde_json::Value>,
    #[serde(default)]
    pub community_sub_gift: Option<serde_json::Value>,
    #[serde(default)]
    pub gift_paid_upgrade: Option<serde_json::Value>,
    #[serde(default)]
    pub prime_paid_upgrade: Option<serde_json::Value>,
    #[serde(default)]
    pub pay_it_forward: Option<serde_json::Value>,
    #[serde(default)]
    pub raid: Option<serde_json::Value>,
    #[serde(default)]
    pub unraid: Option<serde_json::Value>,
    #[serde(default)]
    pub announcement: Option<serde_json::Value>,
    #[serde(default)]
    pub bits_badge_tier: Option<serde_json::Value>,
    #[serde(default)]
    pub charity_donation: Option<serde_json::Value>,
    #[serde(default)]
    pub shared_chat_sub: Option<serde_json::Value>,
    #[serde(default)]
    pub shared_chat_resub: Option<serde_json::Value>,
    #[serde(default)]
    pub shared_chat_sub_gift: Option<serde_json::Value>,
    #[serde(default)]
    pub shared_chat_community_sub_gift: Option<serde_json::Value>,
    #[serde(default)]
    pub shared_chat_gift_paid_upgrade: Option<serde_json::Value>,
    #[serde(default)]
    pub shared_chat_prime_paid_upgrade: Option<serde_json::Value>,
    #[serde(default)]
    pub shared_chat_pay_it_forward: Option<serde_json::Value>,
    #[serde(default)]
    pub shared_chat_raid: Option<serde_json::Value>,
    #[serde(default)]
    pub shared_chat_unraid: Option<serde_json::Value>,
    #[serde(default)]
    pub shared_chat_announcement: Option<serde_json::Value>,
    #[serde(default)]
    pub shared_chat_bits_badge_tier: Option<serde_json::Value>,
    #[serde(default)]
    pub shared_chat_charity_donation: Option<serde_json::Value>,
    #[serde(default)]
    pub source_broadcaster_user_id: Option<String>,
    #[serde(default)]
    pub source_broadcaster_user_login: Option<String>,
    #[serde(default)]
    pub source_broadcaster_user_name: Option<String>,
    #[serde(default)]
    pub source_message_id: Option<String>,
    #[serde(default)]
    pub source_badges: Option<Vec<serde_json::Value>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ChatMessageBody {
    pub text: String,
    #[serde(default)]
    pub fragments: Vec<serde_json::Value>,
}
