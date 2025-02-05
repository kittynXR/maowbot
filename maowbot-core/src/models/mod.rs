// src/models/mod.rs

use std::fmt;
use std::str::FromStr;
use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};
use serde_json::Value;

pub mod user_analysis;
pub use user_analysis::UserAnalysis;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct User {
    pub user_id: String,
    pub global_username: Option<String>,
    pub created_at: DateTime<Utc>,
    pub last_seen: DateTime<Utc>,
    pub is_active: bool,
}

// src/models/mod.rs
#[derive(Debug, Serialize, Deserialize, Clone, Hash, Eq, PartialEq)]
pub enum Platform {
    Twitch,
    Discord,
    VRChat,
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

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum CredentialType {
    OAuth2,
    APIKey,
    BearerToken,
    JWT,
    VerifiableCredential,
    Custom(String),
}

impl fmt::Display for CredentialType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CredentialType::OAuth2 => write!(f, "oauth2"),
            CredentialType::APIKey => write!(f, "apikey"),
            CredentialType::BearerToken => write!(f, "bearer"),
            CredentialType::JWT => write!(f, "jwt"),
            CredentialType::VerifiableCredential => write!(f, "vc"),
            CredentialType::Custom(s) => write!(f, "custom:{}", s),
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
            s if s.starts_with("custom:") => {
                Ok(CredentialType::Custom(s[7..].to_string()))
            }
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

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PlatformCredential {
    pub credential_id: String,
    pub platform: Platform,
    pub credential_type: CredentialType,
    pub user_id: String,
    pub primary_token: String,
    pub refresh_token: Option<String>,
    pub additional_data: Option<serde_json::Value>,
    pub expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub is_bot: bool,
}