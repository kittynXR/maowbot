use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};
use uuid::Uuid;

/// Represents a custom chat command (e.g. `!lurk`) that the bot can handle.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Command {
    pub command_id: Uuid,
    pub platform: String,       // e.g. "twitch-irc", "discord", ...
    pub command_name: String,   // e.g. "!lurk"
    pub min_role: String,       // e.g. "mod", "vip", or "everyone"
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Log record when a user invokes a command.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandUsage {
    pub usage_id: Uuid,
    pub command_id: Uuid,
    pub user_id: Uuid,
    pub used_at: DateTime<Utc>,
    pub channel: String,
    pub usage_text: Option<String>, // the raw text after the command
    pub metadata: Option<serde_json::Value>,
}