use chrono::{DateTime, Utc};
use serde::Deserialize;

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