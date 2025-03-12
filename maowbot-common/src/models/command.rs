use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};
use uuid::Uuid;

/// Represents a custom chat command (e.g. `!lurk`) that the bot can handle.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Command {
    pub command_id: Uuid,
    pub platform: String,
    pub command_name: String,
    pub min_role: String,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,

    pub cooldown_seconds: i32,
    pub cooldown_warnonce: bool,
    pub respond_with_credential: Option<Uuid>,
    pub stream_online_only: bool,
    pub stream_offline_only: bool,
}

/// Log record when a user invokes a command.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandUsage {
    pub usage_id: Uuid,
    pub command_id: Uuid,
    pub user_id: Uuid,
    pub used_at: DateTime<Utc>,
    pub channel: String,
    pub usage_text: Option<String>,
    pub metadata: Option<serde_json::Value>,
}