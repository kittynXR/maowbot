use chrono::{DateTime, Utc};
use serde_json::Value;
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Clone, Debug, FromRow)]
pub struct ChatMessage {
    pub message_id: Uuid,
    pub platform: String,
    pub channel: String,
    pub user_id: Uuid,
    pub message_text: String,
    pub timestamp: DateTime<Utc>,

    // Now stored as JSONB in the DB, so we directly store Option<Value>.
    pub metadata: Option<Value>,
}

#[derive(Clone, Debug)]
pub struct ChatSession {
    pub session_id: Uuid,
    pub platform: String,
    pub channel: String,
    pub user_id: Uuid,
    pub joined_at: DateTime<Utc>,
    pub left_at: Option<DateTime<Utc>>,
    pub session_duration_seconds: Option<i64>,
}

#[derive(Clone, Debug)]
pub struct BotEvent {
    pub event_id: Uuid,
    pub event_type: String,
    pub event_timestamp: DateTime<Utc>,
    pub data: Option<Value>,
}
