// File: maowbot-common/src/models/platform.rs

use std::fmt;
use std::str::FromStr;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;
use crate::models::credential::CredentialType;

/// Add sqlx::Type so that SQLx knows how to decode this enum.
#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq, Hash, sqlx::Type)]
#[sqlx(type_name = "TEXT")]
#[sqlx(rename_all = "lowercase")]
pub enum Platform {
    Twitch,
    Discord,
    VRChat,
    #[sqlx(rename = "twitch-irc")]
    TwitchIRC,
    TwitchEventSub
}

impl fmt::Display for Platform {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Platform::Twitch => write!(f, "twitch"),
            Platform::Discord => write!(f, "discord"),
            Platform::VRChat => write!(f, "vrchat"),
            Platform::TwitchIRC => write!(f, "twitch-irc"),
            Platform::TwitchEventSub => write!(f, "twitch-eventsub"),
        }
    }
}

impl FromStr for Platform {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "twitch" => Ok(Platform::Twitch),
            "discord" => Ok(Platform::Discord),
            "vrchat" => Ok(Platform::VRChat),
            "twitch-irc" => Ok(Platform::TwitchIRC),
            "twitch-eventsub" => Ok(Platform::TwitchEventSub),
            _ => Err(format!("Unknown platform: {}", s)),
        }
    }
}

impl From<String> for Platform {
    fn from(s: String) -> Self {
        s.parse().unwrap_or(Platform::Twitch)
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PlatformIdentity {
    pub platform_identity_id: Uuid,
    pub user_id: Uuid,
    pub platform: Platform,
    pub platform_user_id: String,
    pub platform_username: String,
    pub platform_display_name: Option<String>,
    pub platform_roles: Vec<String>,
    pub platform_data: Value,
    pub created_at: DateTime<Utc>,
    pub last_updated: DateTime<Utc>,
}

/// Add the FromRow derive so SQLx can convert rows to PlatformCredential.
#[derive(Debug, Serialize, Deserialize, Clone, sqlx::FromRow)]
pub struct PlatformCredential {
    pub credential_id: Uuid,
    pub platform: Platform,
    pub platform_id: Option<String>,
    pub credential_type: CredentialType,
    pub user_id: Uuid,
    pub user_name: String,
    pub primary_token: String,
    pub refresh_token: Option<String>,
    pub additional_data: Option<Value>,
    pub expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,

    /// If true, this is the “bot” or automated account. Usually we only keep
    /// one “bot” credential per platform, but that’s not strictly enforced.
    pub is_bot: bool,

    /// **NEW**: If true, this credential is used by a “teammate” (trusted user).
    /// Possibly used for “team mode” where multiple operators can log in
    /// with separate tokens.
    pub is_teammate: bool,

    /// **NEW**: If true, this credential belongs to the broadcaster / owner,
    /// i.e. the main or “primary default” account.
    pub is_broadcaster: bool,
}

#[derive(Debug, Clone)]
pub struct PlatformConfigData {
    pub platform_config_id: uuid::Uuid,
    pub platform: String,
    pub client_id: Option<String>,
    pub client_secret: Option<String>,
}


#[derive(Debug, Clone)]
pub struct PlatformConfig {
    pub platform_config_id: Uuid,
    pub platform: String,
    pub client_id: Option<String>,
    pub client_secret: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl PlatformConfig {
    pub fn new(
        platform: &str,
        client_id: Option<&str>,
        client_secret: Option<&str>
    ) -> Self {
        let now = Utc::now();
        Self {
            platform_config_id: Uuid::new_v4(),
            platform: platform.to_string(),
            client_id: client_id.map(|s| s.to_string()),
            client_secret: client_secret.map(|s| s.to_string()),
            created_at: now,
            updated_at: now,
        }
    }
}
