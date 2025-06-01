// File: maowbot-common/src/models/command.rs

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Represents a custom or built-in command.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Command {
    pub command_id: Uuid,
    pub platform: String,
    pub command_name: String,
    pub min_role: String,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,

    /// Number of seconds between successful invocations
    /// (for spam-limiting).
    pub cooldown_seconds: i32,

    /// If true, user will be warned once that the command is on cooldown
    /// and cannot be used again, but will not spam subsequent warnings
    /// for repeated triggers by the same user in the cooldown period.
    pub cooldown_warnonce: bool,

    /// If set, the command’s “response lines” are sent using
    /// a specific credential. Typically used for Twitch-IRC
    /// so we can choose “bot” or “broadcaster” or a “team” account.
    pub respond_with_credential: Option<Uuid>,

    /// If true, the command only works when the stream is live.
    pub stream_online_only: bool,

    /// If true, the command only works when the stream is offline.
    pub stream_offline_only: bool,

    /// **NEW**: Which credential is “active” for sending, overriding
    /// or clarifying usage. This is used to differentiate from older
    /// `respond_with_credential` logic if desired.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_credential_id: Option<Uuid>,
}

/// Records a single usage of a command by a user.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct CommandUsage {
    pub usage_id: Uuid,
    pub command_id: Uuid,
    pub user_id: Uuid,
    pub used_at: DateTime<Utc>,
    pub channel: String,
    pub usage_text: String,
    pub metadata: Option<serde_json::Value>,
}
