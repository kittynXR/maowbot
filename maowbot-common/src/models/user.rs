use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, Clone, sqlx::FromRow)]
pub struct User {
    pub user_id: Uuid,
    pub global_username: Option<String>,
    pub created_at: DateTime<Utc>,
    pub last_seen: DateTime<Utc>,
    pub is_active: bool,
}

#[derive(Debug, Clone)]
pub struct UserAuditLogEntry {
    pub audit_id: Uuid,
    pub user_id: Uuid,
    pub event_type: String,
    pub old_value: Option<String>,
    pub new_value: Option<String>,
    pub changed_by: Option<String>,
    pub timestamp: DateTime<Utc>,
    pub metadata: Option<String>,
}

impl UserAuditLogEntry {
    pub fn new(
        user_id: Uuid,
        event_type: &str,
        old_value: Option<&str>,
        new_value: Option<&str>,
        changed_by: Option<&str>,
        metadata: Option<&str>,
    ) -> Self {
        Self {
            audit_id: Uuid::new_v4(),
            user_id,
            event_type: event_type.to_string(),
            old_value: old_value.map(String::from),
            new_value: new_value.map(String::from),
            changed_by: changed_by.map(String::from),
            timestamp: Utc::now(),
            metadata: metadata.map(String::from),
        }
    }
}