// File: maowbot-core/src/platforms/twitch_eventsub/events/ban_unban.rs

use serde::Deserialize;
use chrono::{DateTime, Utc};

/// "channel.ban" event
#[derive(Debug, Clone, Deserialize)]
pub struct ChannelBan {
    pub user_id: String,
    pub user_login: String,
    pub user_name: String,
    pub broadcaster_user_id: String,
    pub broadcaster_user_login: String,
    pub broadcaster_user_name: String,
    pub moderator_user_id: String,
    pub moderator_user_login: String,
    pub moderator_user_name: String,
    pub reason: String,
    pub banned_at: DateTime<Utc>,
    pub ends_at: Option<DateTime<Utc>>,
    pub is_permanent: bool,
}

/// "channel.unban" event
#[derive(Debug, Clone, Deserialize)]
pub struct ChannelUnban {
    pub user_id: String,
    pub user_login: String,
    pub user_name: String,
    pub broadcaster_user_id: String,
    pub broadcaster_user_login: String,
    pub broadcaster_user_name: String,
    pub moderator_user_id: String,
    pub moderator_user_login: String,
    pub moderator_user_name: String,
}

/// "channel.unban_request.create" event
#[derive(Debug, Clone, Deserialize)]
pub struct ChannelUnbanRequestCreate {
    pub id: String,
    pub broadcaster_user_id: String,
    pub broadcaster_user_login: String,
    pub broadcaster_user_name: String,
    pub user_id: String,
    pub user_login: String,
    pub user_name: String,
    pub text: String,
    pub created_at: DateTime<Utc>,
}

/// "channel.unban_request.resolve" event
#[derive(Debug, Clone, Deserialize)]
pub struct ChannelUnbanRequestResolve {
    pub id: String,
    pub broadcaster_user_id: String,
    pub broadcaster_user_login: String,
    pub broadcaster_user_name: String,
    pub moderator_user_id: String,
    pub moderator_user_login: String,
    pub moderator_user_name: String,
    pub user_id: String,
    pub user_login: String,
    pub user_name: String,
    pub resolution_text: String,
    pub status: String,
}
