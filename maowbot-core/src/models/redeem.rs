use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};
use uuid::Uuid;

/// Represents a channel point redemption reward configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Redeem {
    pub redeem_id: Uuid,
    pub platform: String,
    pub reward_id: String,
    pub reward_name: String,
    pub cost: i32,
    pub is_active: bool,
    pub dynamic_pricing: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub active_offline: bool,
    pub is_managed: bool,
    pub plugin_name: Option<String>,
    pub command_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedeemUsage {
    pub usage_id: Uuid,
    pub redeem_id: Uuid,
    pub user_id: Uuid,
    pub used_at: DateTime<Utc>,
    pub channel: Option<String>,
    pub usage_data: Option<serde_json::Value>,
}