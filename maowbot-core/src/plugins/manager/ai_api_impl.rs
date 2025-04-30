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
        tracing::info!("🔍 AI_API_IMPL: get_ai_service called, service present: {}", self.ai_service.is_some());
        self.ai_service.clone()
    }
}

#[async_trait]
impl AiApi for AiApiImpl {
    /// Get the AI service for direct operations
    async fn get_ai_service(&self) -> Result<Option<Arc<dyn std::any::Any + Send + Sync>>, Error> {
        tracing::info!("🔍 AI_API_IMPL: AiApi::get_ai_service called, service present: {}", self.ai_service.is_some());
        if let Some(svc) = &self.ai_service {
            tracing::info!("🔍 AI_API_IMPL: AI service is enabled: {}", svc.is_enabled().await);
        }
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
                
                // If we're just updating the model for web search, still pass it through
                // to ensure the provider gets updated properly with the options
                if config.get("default_model").map_or(false, |m| m == "gpt-4o-search-preview") &&
                   config.get("options").map_or(false, |o| o.get("enable_web_search").map_or(false, |v| v == "true")) &&
                   config.get("api_key").is_none() {
                    
                    tracing::info!("Updating model to gpt-4o-search-preview for web search");
                    
                    // Instead of not doing anything, we need to update the provider's options
                    // but keep the existing API key
                    match svc.get_current_provider_config().await {
                        Ok(Some(current_config)) => {
                            // Create a merged config that keeps existing credentials but updates model and options
                            let mut merged_config = current_config.clone();
                            merged_config.default_model = "gpt-4o-search-preview".to_string();
                            
                            // Update or add the web search option
                            merged_config.options.insert("enable_web_search".to_string(), "true".to_string());
                            
                            // Configure with merged config
                            tracing::info!("Applying web search configuration to provider");
                            svc.configure_provider(merged_config).await
                                .map_err(|e| Error::Internal(format!("Failed to configure AI provider for web search: {}", e)))
                        },
                        Ok(None) => {
                            // No current config, can't proceed without API key
                            Err(Error::Internal("Cannot configure web search without existing provider configuration".to_string()))
                        },
                        Err(e) => {
                            Err(Error::Internal(format!("Failed to get current provider config: {}", e)))
                        }
                    }
                } else {
                    // Regular provider configuration - need all fields
                    let provider_config: ProviderConfig = serde_json::from_value(config)
                        .map_err(|e| Error::Internal(format!("Failed to parse provider config: {}", e)))?;
                    
                    svc.configure_provider(provider_config).await
                        .map_err(|e| Error::Internal(format!("Failed to configure AI provider: {}", e)))
                }
            },
            None => Err(Error::Internal("AI service not configured".to_string())),
        }
    }
}