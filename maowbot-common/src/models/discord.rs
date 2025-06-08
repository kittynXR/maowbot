// ========================================================
// File: maowbot-common/src/models/discord.rs
// ========================================================
use chrono::{DateTime, Utc};
/// Represents an entry in our `discord_accounts` table.
#[derive(Debug, Clone)]
pub struct DiscordAccountRecord {
    pub account_name: String,
    pub discord_id: Option<String>,
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
// ------------------------------------------------------------------------------------------------
// NEW: Holds config for specific Discord events (like "stream.online", "stream.offline", etc.)
// ------------------------------------------------------------------------------------------------
#[derive(Debug, Clone)]
pub struct DiscordEventConfigRecord {
    pub event_config_id: uuid::Uuid,
    pub event_name: String,
    pub guild_id: String,
    pub channel_id: String,
    /// If multiple Discord credentials exist, which credential (bot) is used to post?
    /// If null, use whichever default is active.
    pub respond_with_credential: Option<uuid::Uuid>,
    pub ping_roles: Option<Vec<String>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// ------------------------------------------------------------------------------------------------
// Discord LiveRole Record for storing Twitch streamer live role assignment
// ------------------------------------------------------------------------------------------------
#[derive(Debug, Clone)]
pub struct DiscordLiveRoleRecord {
    pub live_role_id: uuid::Uuid,
    pub guild_id: String,
    pub role_id: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// ------------------------------------------------------------------------------------------------
// Discord Embed structures to support rich message formatting
// ------------------------------------------------------------------------------------------------
/// Represents the color of a Discord embed - stored as an integer
#[derive(Debug, Clone)]
pub struct DiscordColor(pub u32);

impl DiscordColor {
    pub const RED: Self = Self(0xED4245);
    pub const GREEN: Self = Self(0x57F287);
    pub const BLUE: Self = Self(0x3498DB);
    pub const PURPLE: Self = Self(0x9B59B6);
    pub const GOLD: Self = Self(0xF1C40F);
    pub const ORANGE: Self = Self(0xE67E22);
    pub const TWITCH_PURPLE: Self = Self(0x6441A5);
    pub const DARK_GREY: Self = Self(0x2C2F33);
}

/// Represents a field in a Discord embed
#[derive(Debug, Clone)]
pub struct DiscordEmbedField {
    pub name: String,
    pub value: String,
    pub inline: bool,
}

/// Represents the author section of a Discord embed
#[derive(Debug, Clone)]
pub struct DiscordEmbedAuthor {
    pub name: String,
    pub url: Option<String>,
    pub icon_url: Option<String>,
}

/// Represents the footer section of a Discord embed
#[derive(Debug, Clone)]
pub struct DiscordEmbedFooter {
    pub text: String,
    pub icon_url: Option<String>,
}

/// Represents an image in a Discord embed
#[derive(Debug, Clone)]
pub struct DiscordEmbedImage {
    pub url: String,
}

/// Represents a thumbnail in a Discord embed
#[derive(Debug, Clone)]
pub struct DiscordEmbedThumbnail {
    pub url: String,
}

/// Main Discord Embed structure that holds all components
#[derive(Debug, Clone)]
pub struct DiscordEmbed {
    pub title: Option<String>,
    pub description: Option<String>,
    pub url: Option<String>,
    pub timestamp: Option<DateTime<Utc>>,
    pub color: Option<DiscordColor>,
    pub footer: Option<DiscordEmbedFooter>,
    pub image: Option<DiscordEmbedImage>,
    pub thumbnail: Option<DiscordEmbedThumbnail>,
    pub author: Option<DiscordEmbedAuthor>,
    pub fields: Vec<DiscordEmbedField>,
}

impl DiscordEmbed {
    pub fn new() -> Self {
        Self {
            title: None,
            description: None,
            url: None,
            timestamp: None,
            color: None,
            footer: None,
            image: None,
            thumbnail: None,
            author: None,
            fields: Vec::new(),
        }
    }
}