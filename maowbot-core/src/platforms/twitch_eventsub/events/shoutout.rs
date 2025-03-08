// File: maowbot-core/src/platforms/twitch_eventsub/events/shoutout.rs

use serde::Deserialize;
use chrono::{DateTime, Utc};

/// "channel.shoutout.create" event
#[derive(Debug, Clone, Deserialize)]
pub struct ChannelShoutoutCreate {
    pub broadcaster_user_id: String,
    pub broadcaster_user_name: String,
    pub broadcaster_user_login: String,
    pub moderator_user_id: String,
    pub moderator_user_name: String,
    pub moderator_user_login: String,
    pub to_broadcaster_user_id: String,
    pub to_broadcaster_user_name: String,
    pub to_broadcaster_user_login: String,
    pub started_at: DateTime<Utc>,
    pub viewer_count: u64,
    pub cooldown_ends_at: DateTime<Utc>,
    pub target_cooldown_ends_at: DateTime<Utc>,
}

/// "channel.shoutout.receive" event
#[derive(Debug, Clone, Deserialize)]
pub struct ChannelShoutoutReceive {
    pub broadcaster_user_id: String,
    pub broadcaster_user_name: String,
    pub broadcaster_user_login: String,
    pub from_broadcaster_user_id: String,
    pub from_broadcaster_user_name: String,
    pub from_broadcaster_user_login: String,
    pub viewer_count: u64,
    pub started_at: DateTime<Utc>,
}
