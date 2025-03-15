// File: maowbot-common/src/models/redeem.rs

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Represents a channel-point style redeem or reward.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Redeem {
    pub redeem_id: Uuid,
    pub platform: String,
    pub reward_id: String,
    pub reward_name: String,
    pub cost: i32,
    pub is_active: bool,
    pub dynamic_pricing: bool,
    pub active_offline: bool,
    pub is_managed: bool,
    pub plugin_name: Option<String>,
    pub command_name: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,

    /// **NEW**: If set, indicates which credential is actually “active”
    /// for this redeem. Could be used for deciding which account processes it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_credential_id: Option<Uuid>,
}

/// Tracks usage of a given redeem by a user.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct RedeemUsage {
    pub usage_id: Uuid,
    pub redeem_id: Uuid,
    pub user_id: Uuid,
    pub used_at: DateTime<Utc>,
    pub channel: Option<String>,
    pub usage_data: Option<serde_json::Value>,
}
