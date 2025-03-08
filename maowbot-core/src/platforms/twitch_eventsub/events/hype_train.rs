// File: maowbot-core/src/platforms/twitch_eventsub/events/hype_train.rs

use serde::Deserialize;
use chrono::{DateTime, Utc};

/// "channel.hype_train.begin" event
#[derive(Debug, Clone, Deserialize)]
pub struct ChannelHypeTrainBegin {
    pub id: String,
    pub broadcaster_user_id: String,
    pub broadcaster_user_login: String,
    pub broadcaster_user_name: String,
    pub total: u64,
    pub progress: u64,
    pub goal: u64,
    #[serde(default)]
    pub top_contributions: Vec<Contribution>,
    pub last_contribution: Contribution,
    pub level: u32,
    pub started_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub is_golden_kappa_train: bool,
}

/// "channel.hype_train.progress" event
#[derive(Debug, Clone, Deserialize)]
pub struct ChannelHypeTrainProgress {
    pub id: String,
    pub broadcaster_user_id: String,
    pub broadcaster_user_login: String,
    pub broadcaster_user_name: String,
    pub level: u32,
    pub total: u64,
    pub progress: u64,
    pub goal: u64,
    #[serde(default)]
    pub top_contributions: Vec<Contribution>,
    pub last_contribution: Contribution,
    pub started_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub is_golden_kappa_train: bool,
}

/// "channel.hype_train.end" event
#[derive(Debug, Clone, Deserialize)]
pub struct ChannelHypeTrainEnd {
    pub id: String,
    pub broadcaster_user_id: String,
    pub broadcaster_user_login: String,
    pub broadcaster_user_name: String,
    pub level: u32,
    pub total: u64,
    #[serde(default)]
    pub top_contributions: Vec<Contribution>,
    pub started_at: DateTime<Utc>,
    pub ended_at: DateTime<Utc>,
    pub cooldown_ends_at: DateTime<Utc>,
    pub is_golden_kappa_train: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Contribution {
    pub user_id: String,
    pub user_login: String,
    pub user_name: String,
    #[serde(rename = "type")]
    pub ctype: String,
    pub total: u64,
}
