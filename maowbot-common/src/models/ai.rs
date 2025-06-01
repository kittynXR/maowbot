// File: maowbot-common/src/models/ai.rs

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;
use std::collections::HashMap;

/// Represents an AI provider
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct AiProvider {
    pub provider_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl AiProvider {
    pub fn new(name: &str, description: Option<&str>) -> Self {
        let now = Utc::now();
        Self {
            provider_id: Uuid::new_v4(),
            name: name.to_string(),
            description: description.map(|s| s.to_string()),
            enabled: true,
            created_at: now,
            updated_at: now,
        }
    }
}

/// Represents a stored API credential for an AI provider
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct AiCredential {
    pub credential_id: Uuid,
    pub provider_id: Uuid,
    pub api_key: String,
    pub api_base: Option<String>,
    pub is_default: bool,
    pub additional_data: Option<Value>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl AiCredential {
    pub fn new(
        provider_id: Uuid,
        api_key: &str,
        api_base: Option<&str>,
        is_default: bool,
        additional_data: Option<Value>,
    ) -> Self {
        let now = Utc::now();
        Self {
            credential_id: Uuid::new_v4(),
            provider_id,
            api_key: api_key.to_string(),
            api_base: api_base.map(|s| s.to_string()),
            is_default,
            additional_data,
            created_at: now,
            updated_at: now,
        }
    }
}

/// Represents an AI model associated with a provider
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct AiModel {
    pub model_id: Uuid,
    pub provider_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub is_default: bool,
    pub capabilities: Option<Value>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl AiModel {
    pub fn new(
        provider_id: Uuid,
        name: &str,
        description: Option<&str>,
        is_default: bool,
        capabilities: Option<Value>,
    ) -> Self {
        let now = Utc::now();
        Self {
            model_id: Uuid::new_v4(),
            provider_id,
            name: name.to_string(),
            description: description.map(|s| s.to_string()),
            is_default,
            capabilities,
            created_at: now,
            updated_at: now,
        }
    }
}

/// Represents an AI agent (MCP)
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct AiAgent {
    pub agent_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub model_id: Uuid,
    pub system_prompt: Option<String>,
    pub capabilities: Option<Value>,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl AiAgent {
    pub fn new(
        name: &str,
        description: Option<&str>,
        model_id: Uuid,
        system_prompt: Option<&str>,
        capabilities: Option<Value>,
        enabled: bool,
    ) -> Self {
        let now = Utc::now();
        Self {
            agent_id: Uuid::new_v4(),
            name: name.to_string(),
            description: description.map(|s| s.to_string()),
            model_id,
            system_prompt: system_prompt.map(|s| s.to_string()),
            capabilities,
            enabled,
            created_at: now,
            updated_at: now,
        }
    }
}

/// Handler types for AI actions
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, sqlx::Type)]
#[sqlx(type_name = "TEXT")]
#[sqlx(rename_all = "lowercase")]
pub enum ActionHandlerType {
    Function,  // Built-in function
    Plugin,    // Custom plugin
    Webhook,   // External API
    Command,   // Bot command
}

impl ToString for ActionHandlerType {
    fn to_string(&self) -> String {
        match self {
            ActionHandlerType::Function => "function".to_string(),
            ActionHandlerType::Plugin => "plugin".to_string(),
            ActionHandlerType::Webhook => "webhook".to_string(),
            ActionHandlerType::Command => "command".to_string(),
        }
    }
}

/// Represents an AI action that can be performed by an agent
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct AiAction {
    pub action_id: Uuid,
    pub agent_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub input_schema: Option<Value>,
    pub output_schema: Option<Value>,
    pub handler_type: String,
    pub handler_config: Option<Value>,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl AiAction {
    pub fn new(
        agent_id: Uuid,
        name: &str,
        description: Option<&str>,
        input_schema: Option<Value>,
        output_schema: Option<Value>,
        handler_type: ActionHandlerType,
        handler_config: Option<Value>,
        enabled: bool,
    ) -> Self {
        let now = Utc::now();
        Self {
            action_id: Uuid::new_v4(),
            agent_id,
            name: name.to_string(),
            description: description.map(|s| s.to_string()),
            input_schema,
            output_schema,
            handler_type: handler_type.to_string(),
            handler_config,
            enabled,
            created_at: now,
            updated_at: now,
        }
    }
}

/// Represents a system prompt template
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct AiSystemPrompt {
    pub prompt_id: Uuid,
    pub name: String,
    pub content: String,
    pub description: Option<String>,
    pub is_default: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl AiSystemPrompt {
    pub fn new(
        name: &str,
        content: &str,
        description: Option<&str>,
        is_default: bool,
    ) -> Self {
        let now = Utc::now();
        Self {
            prompt_id: Uuid::new_v4(),
            name: name.to_string(),
            content: content.to_string(),
            description: description.map(|s| s.to_string()),
            is_default,
            created_at: now,
            updated_at: now,
        }
    }
}

/// Trigger types for AI responses
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, sqlx::Type)]
#[sqlx(type_name = "TEXT")]
#[sqlx(rename_all = "lowercase")]
pub enum TriggerType {
    /// Match at the beginning of a message
    Prefix,
    /// Match using regular expression
    Regex,
    /// Match message that mentions the bot
    Mention,
    /// Scheduled trigger based on time
    Schedule,
    /// Conditional trigger based on events
    Condition,
}

impl ToString for TriggerType {
    fn to_string(&self) -> String {
        match self {
            TriggerType::Prefix => "prefix".to_string(),
            TriggerType::Regex => "regex".to_string(),
            TriggerType::Mention => "mention".to_string(),
            TriggerType::Schedule => "schedule".to_string(),
            TriggerType::Condition => "condition".to_string(),
        }
    }
}

/// Represents a trigger pattern that activates AI responses
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct AiTrigger {
    pub trigger_id: Uuid,
    pub trigger_type: String,
    pub pattern: String,
    pub model_id: Option<Uuid>,
    pub agent_id: Option<Uuid>,
    pub system_prompt: Option<String>,
    pub platform: Option<String>,
    pub channel: Option<String>,
    pub schedule: Option<String>,
    pub condition: Option<String>,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl AiTrigger {
    pub fn new(
        trigger_type: TriggerType,
        pattern: &str,
        model_id: Option<Uuid>,
        agent_id: Option<Uuid>,
        system_prompt: Option<&str>,
        platform: Option<&str>,
        channel: Option<&str>,
        schedule: Option<&str>,
        condition: Option<&str>,
        enabled: bool,
    ) -> Self {
        let now = Utc::now();
        Self {
            trigger_id: Uuid::new_v4(),
            trigger_type: trigger_type.to_string(),
            pattern: pattern.to_string(),
            model_id,
            agent_id,
            system_prompt: system_prompt.map(|s| s.to_string()),
            platform: platform.map(|s| s.to_string()),
            channel: channel.map(|s| s.to_string()),
            schedule: schedule.map(|s| s.to_string()),
            condition: condition.map(|s| s.to_string()),
            enabled,
            created_at: now,
            updated_at: now,
        }
    }
}

/// Role types for AI conversation memory
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, sqlx::Type)]
#[sqlx(type_name = "TEXT")]
#[sqlx(rename_all = "lowercase")]
pub enum MemoryRole {
    System,
    User,
    Assistant,
    Function,
}

impl ToString for MemoryRole {
    fn to_string(&self) -> String {
        match self {
            MemoryRole::System => "system".to_string(),
            MemoryRole::User => "user".to_string(),
            MemoryRole::Assistant => "assistant".to_string(),
            MemoryRole::Function => "function".to_string(),
        }
    }
}

/// Represents a memory entry in an AI conversation
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct AiMemory {
    pub memory_id: Uuid,
    pub user_id: Uuid,
    pub platform: String,
    pub role: String,
    pub content: String,
    pub timestamp: DateTime<Utc>,
    pub metadata: Option<Value>,
}

impl AiMemory {
    pub fn new(
        user_id: Uuid,
        platform: &str,
        role: MemoryRole,
        content: &str,
        metadata: Option<Value>,
    ) -> Self {
        Self {
            memory_id: Uuid::new_v4(),
            user_id,
            platform: platform.to_string(),
            role: role.to_string(),
            content: content.to_string(),
            timestamp: Utc::now(),
            metadata,
        }
    }
}

/// Complete AI configuration containing a provider, credential, and model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiConfiguration {
    pub provider: AiProvider,
    pub credential: AiCredential,
    pub model: AiModel,
}

/// Extended AI trigger with resolved model and provider information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiTriggerWithDetails {
    pub trigger: AiTrigger,
    pub model: Option<AiModel>,
    pub agent: Option<AiAgent>,
    pub provider: Option<AiProvider>,
}

/// Extended AI agent with resolved model and actions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiAgentWithDetails {
    pub agent: AiAgent,
    pub model: AiModel,
    pub provider: AiProvider,
    pub actions: Vec<AiAction>,
}

// Note: The conversion from AiConfiguration to ProviderConfig will be handled
// in the maowbot-core crate to avoid circular dependencies