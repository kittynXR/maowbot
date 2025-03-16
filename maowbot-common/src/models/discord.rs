// ========================================================
// File: maowbot-common/src/models/discord.rs
// ========================================================
use chrono::{DateTime, Utc};

/// Represents an entry in our `discord_accounts` table.
#[derive(Debug, Clone)]
pub struct DiscordAccountRecord {
    pub account_name: String,
    pub credential_id: Option<uuid::Uuid>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Represents a row in the `discord_guilds` table.
#[derive(Debug, Clone)]
pub struct DiscordGuildRecord {
    pub account_name: String,  // which Discord bot account
    pub guild_id: String,      // Discord server ID as a string
    pub guild_name: String,    // Discord server name
    // If you want to treat exactly one guild as "active" for that account, set is_active
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Represents a row in the `discord_channels` table.
#[derive(Debug, Clone)]
pub struct DiscordChannelRecord {
    pub account_name: String,
    pub guild_id: String,
    pub channel_id: String,
    pub channel_name: String,
    // If you want to treat exactly one channel as "active" for that account+guild, set is_active
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
