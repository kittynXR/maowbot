// File: maowbot-core/src/platforms/twitch_eventsub/events/base.rs

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Each subscription wrapper has metadata like `id`, `type`, etc.
/// We'll store it in a generic struct:
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SubscriptionData {
    pub id: String,
    #[serde(rename = "type")]
    pub sub_type: String,
    pub version: String,
    pub status: String,
    pub cost: u32,

    #[serde(default)]
    pub condition: serde_json::Value,

    #[serde(default)]
    pub transport: serde_json::Value,

    #[serde(default)]
    pub created_at: Option<DateTime<Utc>>,
}

/// The top-level wrapper from Twitch for a "notification" message is:
/// { "subscription": { ... }, "event": { ... } }
#[derive(Debug, Clone, Deserialize)]
pub struct EventSubNotificationEnvelope {
    pub subscription: SubscriptionData,
    pub event: serde_json::Value,
}
