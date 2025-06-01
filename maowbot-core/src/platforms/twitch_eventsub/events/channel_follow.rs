// File: maowbot-core/src/platforms/twitch_eventsub/events/channel_follow.rs

use chrono::{DateTime, Utc};
use serde::Deserialize;

/// "channel.follow" event
#[derive(Debug, Clone, Deserialize)]
pub struct ChannelFollow {
    pub user_id: String,
    pub user_login: String,
    pub user_name: String,
    pub broadcaster_user_id: String,
    pub broadcaster_user_login: String,
    pub broadcaster_user_name: String,
    pub followed_at: DateTime<Utc>,
}