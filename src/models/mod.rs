use chrono::{NaiveDateTime};
use serde::{Serialize, Deserialize};
use serde_json::Value;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct User {
    pub user_id: String,
    pub created_at: NaiveDateTime,
    pub last_seen: NaiveDateTime,
    pub is_active: bool,
}

// src/models/mod.rs
#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub enum Platform {
    Twitch,
    Discord,
    VRChat,
}

impl std::fmt::Display for Platform {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Platform::Twitch => write!(f, "twitch"),
            Platform::Discord => write!(f, "discord"),
            Platform::VRChat => write!(f, "vrchat"),
        }
    }
}

impl std::str::FromStr for Platform {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "twitch" => Ok(Platform::Twitch),
            "discord" => Ok(Platform::Discord),
            "vrchat" => Ok(Platform::VRChat),
            _ => Err(format!("Unknown platform: {}", s)),
        }
    }
}

impl From<String> for Platform {
    fn from(s: String) -> Self {
        s.parse().unwrap_or(Platform::Twitch) // You might want different fallback behavior
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
    pub created_at: NaiveDateTime,
    pub last_updated: NaiveDateTime,
}