use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventPipeline {
    pub pipeline_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub enabled: bool,
    pub priority: i32,
    pub stop_on_match: bool,
    pub stop_on_error: bool,
    pub created_by: Option<Uuid>,
    pub is_system: bool,
    pub tags: Vec<String>,
    pub metadata: serde_json::Value,
    pub execution_count: i64,
    pub success_count: i64,
    pub last_executed: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineFilter {
    pub filter_id: Uuid,
    pub pipeline_id: Uuid,
    pub filter_type: String,
    pub filter_config: serde_json::Value,
    pub filter_order: i32,
    pub is_negated: bool,
    pub is_required: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineAction {
    pub action_id: Uuid,
    pub pipeline_id: Uuid,
    pub action_type: String,
    pub action_config: serde_json::Value,
    pub action_order: i32,
    pub continue_on_error: bool,
    pub is_async: bool,
    pub timeout_ms: Option<i32>,
    pub retry_count: i32,
    pub retry_delay_ms: i32,
    pub condition_type: Option<String>,
    pub condition_config: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineExecutionLog {
    pub execution_id: Uuid,
    pub pipeline_id: Uuid,
    pub event_type: String,
    pub event_data: serde_json::Value,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub duration_ms: Option<i32>,
    pub status: PipelineExecutionStatus,
    pub error_message: Option<String>,
    pub actions_executed: i32,
    pub actions_succeeded: i32,
    pub action_results: Vec<ActionExecutionResult>,
    pub triggered_by: Option<Uuid>,
    pub platform: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum PipelineExecutionStatus {
    Running,
    Success,
    Failed,
    Timeout,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionExecutionResult {
    pub action_id: Uuid,
    pub action_type: String,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub duration_ms: Option<i32>,
    pub status: ActionExecutionStatus,
    pub error_message: Option<String>,
    pub output_data: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ActionExecutionStatus {
    Success,
    Failed,
    Timeout,
    Skipped,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineSharedData {
    pub shared_data_id: Uuid,
    pub execution_id: Uuid,
    pub data_key: String,
    pub data_value: serde_json::Value,
    pub data_type: Option<String>,
    pub set_by_action: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventTypeRegistry {
    pub event_type_id: Uuid,
    pub platform: String,
    pub event_category: String,
    pub event_name: String,
    pub description: Option<String>,
    pub event_schema: Option<serde_json::Value>,
    pub is_enabled: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventHandlerRegistry {
    pub handler_id: Uuid,
    pub handler_type: HandlerType,
    pub handler_name: String,
    pub handler_category: String,
    pub description: Option<String>,
    pub parameters: Option<serde_json::Value>,
    pub is_builtin: bool,
    pub plugin_id: Option<String>,
    pub is_enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum HandlerType {
    Filter,
    Action,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreatePipelineRequest {
    pub name: String,
    pub description: Option<String>,
    pub enabled: bool,
    pub priority: i32,
    pub stop_on_match: bool,
    pub stop_on_error: bool,
    pub tags: Vec<String>,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdatePipelineRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub enabled: Option<bool>,
    pub priority: Option<i32>,
    pub stop_on_match: Option<bool>,
    pub stop_on_error: Option<bool>,
    pub tags: Option<Vec<String>>,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateFilterRequest {
    pub filter_type: String,
    pub filter_config: serde_json::Value,
    pub filter_order: i32,
    pub is_negated: bool,
    pub is_required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateActionRequest {
    pub action_type: String,
    pub action_config: serde_json::Value,
    pub action_order: i32,
    pub continue_on_error: bool,
    pub is_async: bool,
    pub timeout_ms: Option<i32>,
    pub retry_count: i32,
    pub retry_delay_ms: i32,
    pub condition_type: Option<String>,
    pub condition_config: Option<serde_json::Value>,
}

impl Default for EventPipeline {
    fn default() -> Self {
        Self {
            pipeline_id: Uuid::new_v4(),
            name: String::new(),
            description: None,
            enabled: true,
            priority: 100,
            stop_on_match: false,
            stop_on_error: false,
            created_by: None,
            is_system: false,
            tags: Vec::new(),
            metadata: serde_json::Value::Object(serde_json::Map::new()),
            execution_count: 0,
            success_count: 0,
            last_executed: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }
}

impl EventPipeline {
    pub fn success_rate(&self) -> f64 {
        if self.execution_count == 0 {
            0.0
        } else {
            (self.success_count as f64) / (self.execution_count as f64)
        }
    }

    pub fn is_user_created(&self) -> bool {
        !self.is_system && self.created_by.is_some()
    }
}

impl PipelineFilter {
    pub fn applies_to_event(&self, event_type: &str, event_data: &serde_json::Value) -> bool {
        // This would be implemented based on the filter type
        // For now, return true as placeholder
        true
    }
}

impl PipelineAction {
    pub fn should_execute(&self, previous_result: Option<&ActionExecutionResult>) -> bool {
        match &self.condition_type {
            None => true,
            Some(condition) => match condition.as_str() {
                "previous_success" => previous_result.map_or(true, |r| r.status == ActionExecutionStatus::Success),
                "previous_failure" => previous_result.map_or(false, |r| r.status == ActionExecutionStatus::Failed),
                _ => true,
            }
        }
    }
}