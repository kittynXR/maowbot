// File: maowbot-core/src/platforms/twitch_eventsub/events/subscription.rs

use serde::Deserialize;

/// "channel.subscribe" event
#[derive(Debug, Clone, Deserialize)]
pub struct ChannelSubscribe {
    pub user_id: String,
    pub user_login: String,
    pub user_name: String,
    pub broadcaster_user_id: String,
    pub broadcaster_user_login: String,
    pub broadcaster_user_name: String,
    pub tier: String,
    pub is_gift: bool,
}

/// "channel.subscription.end" event
#[derive(Debug, Clone, Deserialize)]
pub struct ChannelSubscriptionEnd {
    pub user_id: String,
    pub user_login: String,
    pub user_name: String,
    pub broadcaster_user_id: String,
    pub broadcaster_user_login: String,
    pub broadcaster_user_name: String,
    pub tier: String,
    pub is_gift: bool,
}

/// "channel.subscription.gift" event
#[derive(Debug, Clone, Deserialize)]
pub struct ChannelSubscriptionGift {
    pub user_id: String,
    pub user_login: String,
    pub user_name: String,
    pub broadcaster_user_id: String,
    pub broadcaster_user_login: String,
    pub broadcaster_user_name: String,
    pub total: u64,
    pub tier: String,
    pub cumulative_total: Option<u64>,
    pub is_anonymous: bool,
}

/// "channel.subscription.message" event
#[derive(Debug, Clone, Deserialize)]
pub struct ChannelSubscriptionMessage {
    pub user_id: String,
    pub user_login: String,
    pub user_name: String,
    pub broadcaster_user_id: String,
    pub broadcaster_user_login: String,
    pub broadcaster_user_name: String,
    pub tier: String,
    pub message: SubMessage,
    pub cumulative_months: u32,
    pub streak_months: Option<u32>,
    pub duration_months: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SubMessage {
    pub text: String,
    #[serde(default)]
    pub emotes: Vec<SubEmote>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SubEmote {
    pub begin: u32,
    pub end: u32,
    pub id: String,
}
