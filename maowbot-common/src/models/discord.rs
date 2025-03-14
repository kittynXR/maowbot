use chrono::{DateTime, Utc};

#[derive(Debug, Clone)]
pub struct DiscordGuildRecord {
    pub account_name: String,  // which Discord bot account
    pub guild_id: String,      // Discord server ID as a string
    pub guild_name: String,    // Discord server name
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct DiscordChannelRecord {
    pub account_name: String,
    pub guild_id: String,
    pub channel_id: String,
    pub channel_name: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}