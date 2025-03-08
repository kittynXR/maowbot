// File: maowbot-core/src/platforms/twitch_eventsub/events/channel_follow_update.rs

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

/// "channel.update" event
#[derive(Debug, Clone, Deserialize)]
pub struct ChannelUpdate {
    pub broadcaster_user_id: String,
    pub broadcaster_user_login: String,
    pub broadcaster_user_name: String,
    pub title: String,
    pub language: String,
    pub category_id: String,
    pub category_name: String,
    #[serde(default)]
    pub content_classification_labels: Vec<String>,
}

/// "channel.ad_break.begin" event
#[derive(Debug, Clone, Deserialize)]
pub struct ChannelAdBreakBegin {
    pub duration_seconds: String,
    pub started_at: DateTime<Utc>,
    pub is_automatic: String,
    pub broadcaster_user_id: String,
    pub broadcaster_user_login: String,
    pub broadcaster_user_name: String,
    pub requester_user_id: String,
    pub requester_user_login: String,
    pub requester_user_name: String,
}
