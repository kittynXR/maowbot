// File: maowbot-core/src/models/mod.rs

use std::fmt;
use std::str::FromStr;
use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};
use serde_json::Value;
use sqlx::FromRow;

pub mod user_analysis;
pub use user_analysis::UserAnalysis;

#[derive(Debug, Serialize, Deserialize, Clone, FromRow)]
pub struct User {
    pub user_id: String,
    pub global_username: Option<String>,
    pub created_at: DateTime<Utc>,
    pub last_seen: DateTime<Utc>,
    pub is_active: bool,
}

/// Add sqlx::Type so that SQLx knows how to decode this enum.
/// Here we tell SQLx that the enum is stored as TEXT.
#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq, Hash, sqlx::Type)]
#[sqlx(type_name = "TEXT")]
#[sqlx(rename_all = "lowercase")]
pub enum Platform {
    Twitch,
    Discord,
    VRChat,
    #[sqlx(rename = "twitch-irc")]
    TwitchIRC,
}

impl fmt::Display for Platform {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Platform::Twitch => write!(f, "twitch"),
            Platform::Discord => write!(f, "discord"),
            Platform::VRChat => write!(f, "vrchat"),
            Platform::TwitchIRC => write!(f, "twitch-irc"),
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
            _ => Err(format!("Unknown platform: {}", s)),
        }
    }
}

impl From<String> for Platform {
    fn from(s: String) -> Self {
        s.parse().unwrap_or(Platform::Twitch)
    }
}

/// CredentialType now supports only the known types.
/// We have removed the old data-bearing custom variant and replaced it
/// with a unit variant for interactive 2FA.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, sqlx::Type)]
#[sqlx(type_name = "TEXT")]
pub enum CredentialType {
    OAuth2,
    APIKey,
    BearerToken,
    JWT,
    VerifiableCredential,
    Interactive2FA,
}

impl fmt::Display for CredentialType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CredentialType::OAuth2 => write!(f, "oauth2"),
            CredentialType::APIKey => write!(f, "apikey"),
            CredentialType::BearerToken => write!(f, "bearer"),
            CredentialType::JWT => write!(f, "jwt"),
            CredentialType::VerifiableCredential => write!(f, "vc"),
            CredentialType::Interactive2FA => write!(f, "interactive2fa"),
        }
    }
}

impl FromStr for CredentialType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "oauth2" => Ok(CredentialType::OAuth2),
            "apikey" => Ok(CredentialType::APIKey),
            "bearer" => Ok(CredentialType::BearerToken),
            "jwt" => Ok(CredentialType::JWT),
            "vc" => Ok(CredentialType::VerifiableCredential),
            "interactive2fa" | "i2fa" => Ok(CredentialType::Interactive2FA),
            _ => Err(format!("Invalid credential type: {}", s))
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PlatformIdentity {
    pub platform_identity_id: String,
    pub user_id: String,
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
#[derive(Debug, Serialize, Deserialize, Clone, FromRow)]
pub struct PlatformCredential {
    pub credential_id: String,
    pub platform: Platform,
    pub credential_type: CredentialType,
    pub user_id: String,
    pub primary_token: String,
    pub refresh_token: Option<String>,
    pub additional_data: Option<Value>,
    pub expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub is_bot: bool,
}