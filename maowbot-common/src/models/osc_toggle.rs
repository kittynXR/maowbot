use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct OscTrigger {
    pub id: i32,
    pub redeem_id: Uuid,
    pub parameter_name: String,
    pub parameter_type: String,
    pub on_value: String,
    pub off_value: String,
    pub duration_seconds: Option<i32>,
    pub cooldown_seconds: i32,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct OscToggleState {
    pub id: i32,
    pub trigger_id: i32,
    pub user_id: Uuid,
    pub avatar_id: Option<String>,
    pub activated_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub is_active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct OscAvatarConfig {
    pub id: i32,
    pub avatar_id: String,
    pub avatar_name: Option<String>,
    pub parameter_mappings: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OscParameterValue {
    Bool(bool),
    Int(i32),
    Float(f32),
}

impl OscParameterValue {
    pub fn from_string(value_type: &str, value: &str) -> Result<Self, String> {
        match value_type {
            "bool" => {
                let v: bool = serde_json::from_str(value)
                    .map_err(|e| format!("Failed to parse bool: {}", e))?;
                Ok(Self::Bool(v))
            }
            "int" => {
                let v: i32 = serde_json::from_str(value)
                    .map_err(|e| format!("Failed to parse int: {}", e))?;
                Ok(Self::Int(v))
            }
            "float" => {
                let v: f32 = serde_json::from_str(value)
                    .map_err(|e| format!("Failed to parse float: {}", e))?;
                Ok(Self::Float(v))
            }
            _ => Err(format!("Unknown parameter type: {}", value_type)),
        }
    }
}