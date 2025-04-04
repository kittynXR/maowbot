use std::sync::Arc;

use async_trait::async_trait;
use uuid::Uuid;
use maowbot_common::traits::api::AiApi;
use maowbot_common::error::Error;
use maowbot_ai::plugins::ai_service::AiService;
use serde_json::Value;

/// Implementation of the AiApi trait for the PluginManager
#[derive(Clone)]
pub struct AiApiImpl {
    ai_service: Option<Arc<AiService>>,
}

impl AiApiImpl {
    /// Create a new AiApiImpl with the given AiService
    pub fn new(ai_service: Arc<AiService>) -> Self {
        Self { ai_service: Some(ai_service) }
    }
    
    /// Create a stub implementation that returns errors
    pub fn new_stub() -> Self {
        Self { ai_service: None }
    }
    
    /// Get a reference to the underlying AiService
    pub fn get_ai_service(&self) -> Option<Arc<AiService>> {
        self.ai_service.clone()
    }
}

#[async_trait]
impl AiApi for AiApiImpl {
    /// Get the AI service for direct operations
    async fn get_ai_service(&self) -> Result<Option<Arc<dyn std::any::Any + Send + Sync>>, Error> {
        Ok(self.ai_service.clone().map(|svc| svc as Arc<dyn std::any::Any + Send + Sync>))
    }

    /// Generate a chat completion
    async fn generate_chat(&self, messages: Vec<Value>) -> Result<String, Error> {
        match &self.ai_service {
            Some(svc) => svc.process_chat_message_raw(messages).await
                .map_err(|e| Error::Internal(format!("AI error: {}", e))),
            None => Err(Error::Internal("AI service not configured".to_string())),
        }
    }
    
    /// Generate a completion with function calling
    async fn generate_with_functions(&self, messages: Vec<Value>) -> Result<Value, Error> {
        match &self.ai_service {
            Some(svc) => svc.process_chat_with_functions(messages).await
                .map_err(|e| Error::Internal(format!("AI error: {}", e))),
            None => Err(Error::Internal("AI service not configured".to_string())),
        }
    }
    
    /// Process a user message with context
    async fn process_user_message(&self, user_id: Uuid, message: &str) -> Result<String, Error> {
        match &self.ai_service {
            Some(svc) => svc.process_user_message(user_id, message).await
                .map_err(|e| Error::Internal(format!("AI error: {}", e))),
            None => Err(Error::Internal("AI service not configured".to_string())),
        }
    }
    
    /// Register a new function
    async fn register_ai_function(&self, name: &str, description: &str) -> Result<(), Error> {
        match &self.ai_service {
            Some(svc) => svc.register_function(name, description).await
                .map_err(|e| Error::Internal(format!("AI error: {}", e))),
            None => Err(Error::Internal("AI service not configured".to_string())),
        }
    }
    
    /// Set the system prompt
    async fn set_system_prompt(&self, prompt: &str) -> Result<(), Error> {
        match &self.ai_service {
            Some(svc) => svc.set_system_prompt(prompt).await
                .map_err(|e| Error::Internal(format!("AI error: {}", e))),
            None => Err(Error::Internal("AI service not configured".to_string())),
        }
    }
    
    /// Configure an AI provider with the given configuration
    async fn configure_ai_provider(&self, config: Value) -> Result<(), Error> {
        match &self.ai_service {
            Some(svc) => {
                // Try to deserialize the config into a ProviderConfig
                use maowbot_ai::models::ProviderConfig;
                
                let provider_config: ProviderConfig = serde_json::from_value(config)
                    .map_err(|e| Error::Internal(format!("Failed to parse provider config: {}", e)))?;
                
                svc.configure_provider(provider_config).await
                    .map_err(|e| Error::Internal(format!("Failed to configure AI provider: {}", e)))
            },
            None => Err(Error::Internal("AI service not configured".to_string())),
        }
    }
}