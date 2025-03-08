use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

/// Matches the "stream.online" notification payload:
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamOnline {
    pub id: String,                       // e.g. "9001"
    pub broadcaster_user_id: String,      // e.g. "1337"
    pub broadcaster_user_login: String,   // e.g. "cool_user"
    pub broadcaster_user_name: String,    // e.g. "Cool_User"
    #[serde(default)]
    pub r#type: String,                   // e.g. "live"
    pub started_at: DateTime<Utc>,
}

/// Matches the "stream.offline" notification payload:
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamOffline {
    pub broadcaster_user_id: String,
    pub broadcaster_user_login: String,
    pub broadcaster_user_name: String,
}
