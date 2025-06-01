use serde::{Serialize, Deserialize};
use std::collections::HashMap;

/// Configuration for an AI provider
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    /// The type of provider (OpenAI, Anthropic, etc.)
    pub provider_type: String,
    
    /// Base URL for API requests
    pub api_base: Option<String>,
    
    /// API key for authentication
    pub api_key: String,
    
    /// Default model to use with this provider
    pub default_model: String,
    
    /// Additional provider-specific configuration options
    pub options: HashMap<String, String>,
}

/// Represents an AI function parameter
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionParameter {
    /// Name of the parameter
    pub name: String,
    
    /// Description of the parameter's purpose
    pub description: String,
    
    /// Data type of the parameter (string, number, boolean, etc.)
    pub parameter_type: String,
    
    /// Whether this parameter is required
    pub required: bool,
    
    /// Default value if parameter is not provided
    pub default: Option<serde_json::Value>,
    
    /// For enum types, the possible values
    pub enum_values: Option<Vec<String>>,
}

/// Represents a user interaction memory entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    /// ID of the user this memory belongs to
    pub user_id: String,
    
    /// Platform where the interaction happened (Twitch, Discord, etc.)
    pub platform: String,
    
    /// Timestamp of when this memory was created
    pub timestamp: chrono::DateTime<chrono::Utc>,
    
    /// The message content
    pub message: crate::traits::ChatMessage,
    
    /// Additional context or metadata
    pub metadata: HashMap<String, String>,
}