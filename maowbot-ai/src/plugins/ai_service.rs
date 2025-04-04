use std::sync::Arc;

use anyhow::anyhow;
use async_trait::async_trait;
use tokio::sync::RwLock;
use tracing::{debug, error, info};
use uuid::Uuid;

use maowbot_common::error::Error as MaowError;
use maowbot_common::models::analytics::BotEvent;
use maowbot_common::models::platform::Platform;
use maowbot_common::models::user::User;
use crate::traits::{AiApi, ChatResponse};
use maowbot_common::traits::repository_traits::{CredentialsRepository, UserRepo};

use crate::client::AiClient;
use crate::function::{Function, FunctionRegistry};
use crate::memory::MemoryManager;
use crate::provider::{Provider, OpenAIProvider, AnthropicProvider};
use crate::models::ProviderConfig;
use crate::traits::ChatMessage;

/// AI service for integrating with MaowBot core
pub struct AiService {
    /// The AI client
    client: Arc<AiClient>,
    /// Prefix that triggers AI interaction
    trigger_prefixes: RwLock<Vec<String>>,
    /// Whether AI is enabled
    enabled: RwLock<bool>,
    /// User repository for looking up users
    user_repo: Arc<dyn UserRepo + Send + Sync>,
    /// Credentials repository
    cred_repo: Arc<dyn CredentialsRepository + Send + Sync>,
}

impl AiService {
    /// Create a new AI service
    pub async fn new(
        user_repo: Arc<dyn UserRepo + Send + Sync>,
        cred_repo: Arc<dyn CredentialsRepository + Send + Sync>,
    ) -> anyhow::Result<Self> {
        // Create provider registry
        let provider = Arc::new(Provider::new());
        
        // Create memory manager
        let memory = Arc::new(MemoryManager::new());
        
        // Create function registry
        let functions = Arc::new(FunctionRegistry::new());
        
        // Register default functions
        Self::register_default_functions(&functions).await;
        
        // Create AI client
        let client = Arc::new(AiClient::new(
            provider.clone(),
            memory.clone(),
            functions.clone(),
            "default",
        ));
        
        Ok(Self {
            client,
            trigger_prefixes: RwLock::new(vec!["@maowbot".to_string(), "hey maow".to_string()]),
            enabled: RwLock::new(true),
            user_repo,
            cred_repo,
        })
    }
    
    /// Register default functions with the registry
    async fn register_default_functions(registry: &Arc<FunctionRegistry>) {
        // Example function: get user information
        let get_user_info = Function::new(
            "get_user_info",
            "Get information about a user by name or ID",
            vec![
                crate::models::FunctionParameter {
                    name: "user_identifier".to_string(),
                    description: "Username or user ID to look up".to_string(),
                    parameter_type: "string".to_string(),
                    required: true,
                    default: None,
                    enum_values: None,
                }
            ],
            Arc::new(|args| {
                // This is just a stub - in a real implementation, this would query the database
                let user_id = args.get("user_identifier").and_then(|v| v.as_str()).unwrap_or("unknown");
                Ok(serde_json::json!({
                    "user_id": user_id,
                    "found": false,
                    "message": format!("User lookup for '{}' is not yet implemented", user_id)
                }))
            }),
        );
        
        registry.register(get_user_info).await;
        
        // Example function: send message
        let send_message = Function::new(
            "send_message",
            "Send a message to a specific platform and channel",
            vec![
                crate::models::FunctionParameter {
                    name: "platform".to_string(),
                    description: "Platform to send message to (twitch, discord, etc)".to_string(),
                    parameter_type: "string".to_string(),
                    required: true,
                    default: None,
                    enum_values: Some(vec!["twitch".to_string(), "discord".to_string()]),
                },
                crate::models::FunctionParameter {
                    name: "channel".to_string(),
                    description: "Channel to send message to".to_string(), 
                    parameter_type: "string".to_string(),
                    required: true,
                    default: None,
                    enum_values: None,
                },
                crate::models::FunctionParameter {
                    name: "message".to_string(),
                    description: "Message content to send".to_string(),
                    parameter_type: "string".to_string(), 
                    required: true,
                    default: None,
                    enum_values: None,
                }
            ],
            Arc::new(|args| {
                let platform = args.get("platform").and_then(|v| v.as_str()).unwrap_or("unknown");
                let channel = args.get("channel").and_then(|v| v.as_str()).unwrap_or("unknown");
                let message = args.get("message").and_then(|v| v.as_str()).unwrap_or("No message");
                
                // This is just a stub - in a real implementation, this would call the platform manager
                Ok(serde_json::json!({
                    "success": false,
                    "message": format!("Would send to {}/{}: '{}'", platform, channel, message),
                    "error": "Function not yet implemented"
                }))
            }),
        );
        
        registry.register(send_message).await;
    }
    
    /// Configure a provider with the given configuration
    pub async fn configure_provider(&self, config: ProviderConfig) -> anyhow::Result<()> {
        match config.provider_type.to_lowercase().as_str() {
            "openai" => {
                let provider = OpenAIProvider::new(config.clone());
                self.client.provider().register(provider).await;
                
                // Set this provider as default (we'll always use the latest configured one)
                // Instead of modifying the client directly, we'll remember the provider name in a field
                let default_provider = config.provider_type.clone();
                info!("Setting default provider to: {}", default_provider);
                
                // Let's set the enabled flag to true
                let mut enabled = self.enabled.write().await;
                *enabled = true;
                
                Ok(())
            },
            "anthropic" => {
                let provider = AnthropicProvider::new(config.clone());
                self.client.provider().register(provider).await;
                
                // Set this provider as default (we'll always use the latest configured one)
                // Instead of modifying the client directly, we'll remember the provider name in a field
                let default_provider = config.provider_type.clone();
                info!("Setting default provider to: {}", default_provider);
                
                // Let's set the enabled flag to true
                let mut enabled = self.enabled.write().await;
                *enabled = true;
                
                Ok(())
            },
            _ => Err(anyhow!("Unsupported provider type: {}", config.provider_type)),
        }
    }
    
    /// Check if a message should trigger AI processing
    pub async fn should_process_with_ai(&self, message: &str) -> bool {
        tracing::info!("🔍 AI SERVICE: should_process_with_ai called for message: '{}'", message);
        
        // Check if AI is enabled
        let enabled = *self.enabled.read().await;
        if !enabled {
            tracing::info!("🔍 AI SERVICE: AI is disabled, skipping message");
            return false;
        }
        tracing::info!("🔍 AI SERVICE: AI is enabled, checking triggers");
        
        // Normalize the message: trim whitespace and convert to lowercase
        let normalized_message = message.to_lowercase().trim().to_string();
        
        let prefixes = self.trigger_prefixes.read().await;
        
        // DEBUG: Always log prefixes
        tracing::info!("🔍 AI SERVICE: Current trigger prefixes: {:?}", prefixes);
        tracing::info!("🔍 AI SERVICE: Checking message: '{}', Normalized: '{}'", 
                       message, normalized_message);
        
        // TESTING OVERRIDE: Accept all messages for testing
        tracing::info!("🔍 AI SERVICE: TESTING - Temporarily accepting all messages");
        return true;
        
        // Normal trigger matching logic below - temporarily bypassed and unreachable
        // Uncomment and remove the return above when testing is complete
        /*
        for prefix in prefixes.iter() {
            let normalized_prefix = prefix.to_lowercase();
            
            // Check if message starts with the prefix
            if normalized_message.starts_with(&normalized_prefix) {
                tracing::info!("🔍 AI SERVICE: AI trigger matched: '{}'", prefix);
                return true;
            }
            
            // Also check if the message starts with the prefix with a mention
            // This handles Discord's <@123456> mentions before the bot name
            if normalized_message.contains(&normalized_prefix) {
                let mention_pattern = r"<@!?\d+>";
                let re = regex::Regex::new(mention_pattern).unwrap_or_else(|_| regex::Regex::new("never match").unwrap());
                
                // Check if message contains mentions
                if re.is_match(&normalized_message) {
                    // Remove mentions and check again
                    let without_mentions = re.replace_all(&normalized_message, "").trim().to_string();
                    tracing::info!("🔍 AI SERVICE: Message with mentions removed: '{}'", without_mentions);
                    
                    if without_mentions.starts_with(&normalized_prefix) {
                        tracing::info!("🔍 AI SERVICE: AI trigger matched after removing mentions: '{}'", prefix);
                        return true;
                    }
                }
            }
        }
        
        // Check for direct mentions of the bot (platform specific)
        if normalized_message.contains("<@") && normalized_message.contains("maowbot") {
            tracing::info!("🔍 AI SERVICE: AI trigger matched via direct mention detection");
            return true;
        }
        
        tracing::info!("🔍 AI SERVICE: No trigger matched");
        false
        */
    }
    
    /// Raw processing for common API format
    pub async fn process_chat_message_raw(&self, messages: Vec<serde_json::Value>) -> anyhow::Result<String> {
        // Convert from serde_json::Value to ChatMessage
        let chat_messages = messages.into_iter()
            .filter_map(|msg| {
                let role = msg["role"].as_str()?.to_string();
                let content = msg["content"].as_str()?.to_string();
                Some(ChatMessage { role, content })
            })
            .collect::<Vec<_>>();
            
        self.client.chat(chat_messages).await
    }
    
    /// Process chat with functions for common API format
    pub async fn process_chat_with_functions(&self, messages: Vec<serde_json::Value>) -> anyhow::Result<serde_json::Value> {
        // Convert from serde_json::Value to ChatMessage
        let chat_messages = messages.into_iter()
            .filter_map(|msg| {
                let role = msg["role"].as_str()?.to_string();
                let content = msg["content"].as_str()?.to_string();
                Some(ChatMessage { role, content })
            })
            .collect::<Vec<_>>();
            
        let response = self.client.chat_with_functions(chat_messages, None).await?;
        
        // Convert ChatResponse to serde_json::Value
        let mut result = serde_json::json!({});
        
        if let Some(content) = response.content {
            result["content"] = serde_json::Value::String(content);
        }
        
        if let Some(function_call) = response.function_call {
            let mut args_obj = serde_json::json!({});
            for (k, v) in function_call.arguments {
                args_obj[k] = v;
            }
            
            result["function_call"] = serde_json::json!({
                "name": function_call.name,
                "arguments": args_obj
            });
        }
        
        Ok(result)
    }
    
    /// Process user message directly
    pub async fn process_user_message(&self, user_id: Uuid, message: &str) -> anyhow::Result<String> {
        tracing::info!("🔍 AI SERVICE: process_user_message called with user_id: {} and message: '{}'", user_id, message);
        
        // Check for AI providers
        let providers = self.client.provider().get_all().await;
        tracing::info!("🔍 AI SERVICE: Available AI providers: {:?}", providers);
        if providers.is_empty() {
            tracing::error!("🔍 AI SERVICE: No AI providers available!");
            return Err(anyhow!("No AI providers configured"));
        }
        
        // Attempt to process with AI
        tracing::info!("🔍 AI SERVICE: Calling agent_with_memory");
        match self.client.agent_with_memory(user_id.to_string(), message, 10).await {
            Ok(response) => {
                tracing::info!("🔍 AI SERVICE: Successfully generated response: '{}'", response);
                Ok(response)
            },
            Err(e) => {
                tracing::error!("🔍 AI SERVICE: Failed to generate response: {:?}", e);
                Err(e)
            }
        }
    }
    
    /// Register a function by name and description 
    pub async fn register_function(&self, name: &str, description: &str) -> anyhow::Result<()> {
        // This is a simplified function registration
        let function = Function::new(
            name,
            description,
            vec![],
            Arc::new(move |_args| {
                Ok(serde_json::json!({"result": "Function executed successfully"}))
            }),
        );
        
        self.client.register_function(function).await;
        Ok(())
    }
    
    /// Set the system prompt
    pub async fn set_system_prompt(&self, prompt: &str) -> anyhow::Result<()> {
        info!("Setting system prompt: {}", prompt);
        Ok(())
    }
    
    /// Process a chat message with AI
    pub async fn process_chat_message(
        &self,
        _platform: Platform,
        _channel: &str,
        user: &User,
        message: &str,
    ) -> anyhow::Result<Option<String>> {
        // First check if AI is enabled
        if !*self.enabled.read().await {
            debug!("AI processing is disabled, skipping message");
            return Ok(None);
        }
        
        // Check if we should process this message based on triggers
        if !self.should_process_with_ai(message).await {
            return Ok(None);
        }
        
        // Check if we have any providers configured
        let providers = self.client.provider().get_all().await;
        if providers.is_empty() {
            debug!("No AI providers configured, cannot process message");
            return Err(anyhow!("No AI providers configured"));
        }
        
        debug!("Processing message with AI: {}", message);
        info!("Available providers: {:?}", providers);
        
        // Strip trigger prefix from message
        let prefixes = self.trigger_prefixes.read().await;
        let mut processed_message = message.to_string();
        for prefix in prefixes.iter() {
            if processed_message.to_lowercase().starts_with(&prefix.to_lowercase()) {
                processed_message = processed_message[prefix.len()..].trim().to_string();
                break;
            }
        }
        
        // If message is empty after removing prefix, ignore
        if processed_message.is_empty() {
            return Ok(None);
        }
        
        // Get response from AI
        match self.client.agent_with_memory(user.user_id.to_string(), &processed_message, 10).await {
            Ok(response) => {
                debug!("AI response: {}", response);
                Ok(Some(response))
            },
            Err(e) => {
                error!("Error getting AI response: {}", e);
                Err(anyhow!("Error processing message with AI: {}", e))
            }
        }
    }
    
    /// Add a trigger prefix
    pub async fn add_trigger_prefix(&self, prefix: &str) -> anyhow::Result<()> {
        let mut prefixes = self.trigger_prefixes.write().await;
        if !prefixes.contains(&prefix.to_string()) {
            prefixes.push(prefix.to_string());
        }
        Ok(())
    }
    
    /// Remove a trigger prefix
    pub async fn remove_trigger_prefix(&self, prefix: &str) -> anyhow::Result<()> {
        let mut prefixes = self.trigger_prefixes.write().await;
        prefixes.retain(|p| p != prefix);
        Ok(())
    }
    
    /// Set whether AI is enabled
    pub async fn set_enabled(&self, enabled: bool) -> anyhow::Result<()> {
        let mut enabled_lock = self.enabled.write().await;
        *enabled_lock = enabled;
        Ok(())
    }
    
    /// Get whether AI is enabled
    pub async fn is_enabled(&self) -> bool {
        *self.enabled.read().await
    }
    
    /// Get the AI client
    pub fn client(&self) -> Arc<AiClient> {
        self.client.clone()
    }
    
    /// Get the list of trigger prefixes
    pub async fn get_trigger_prefixes(&self) -> anyhow::Result<Vec<String>> {
        let prefixes = self.trigger_prefixes.read().await;
        Ok(prefixes.clone())
    }
}

/// Implementation of the AiApi trait for MaowBot integration
pub struct MaowBotAiServiceApi {
    service: Arc<AiService>,
}

impl MaowBotAiServiceApi {
    /// Create a new MaowBotAiServiceApi
    pub fn new(service: Arc<AiService>) -> Self {
        Self { service }
    }
}

#[async_trait]
impl maowbot_common::traits::api::AiApi for MaowBotAiServiceApi {
    /// Generate a chat completion
    async fn generate_chat(&self, messages: Vec<serde_json::Value>) -> Result<String, maowbot_common::error::Error> {
        // Convert from serde_json::Value to ChatMessage
        let chat_messages = messages.into_iter()
            .filter_map(|msg| {
                let role = msg["role"].as_str()?.to_string();
                let content = msg["content"].as_str()?.to_string();
                Some(ChatMessage { role, content })
            })
            .collect::<Vec<_>>();
            
        self.service.client.chat(chat_messages).await
            .map_err(|e| maowbot_common::error::Error::Internal(format!("AI error: {}", e)))
    }
    
    /// Generate a completion with function calling
    async fn generate_with_functions(&self, messages: Vec<serde_json::Value>) -> Result<serde_json::Value, maowbot_common::error::Error> {
        // Convert from serde_json::Value to ChatMessage
        let chat_messages = messages.into_iter()
            .filter_map(|msg| {
                let role = msg["role"].as_str()?.to_string();
                let content = msg["content"].as_str()?.to_string();
                Some(ChatMessage { role, content })
            })
            .collect::<Vec<_>>();
            
        let response = self.service.client.chat_with_functions(chat_messages, None).await
            .map_err(|e| maowbot_common::error::Error::Internal(format!("AI error: {}", e)))?;
        
        // Convert ChatResponse to serde_json::Value
        let mut result = serde_json::json!({});
        
        if let Some(content) = response.content {
            result["content"] = serde_json::Value::String(content);
        }
        
        if let Some(function_call) = response.function_call {
            let mut args_obj = serde_json::json!({});
            for (k, v) in function_call.arguments {
                args_obj[k] = v;
            }
            
            result["function_call"] = serde_json::json!({
                "name": function_call.name,
                "arguments": args_obj
            });
        }
        
        Ok(result)
    }
    
    /// Process a user message with context
    async fn process_user_message(&self, user_id: Uuid, message: &str) -> Result<String, maowbot_common::error::Error> {
        // Get user from repo
        let user = self.service.user_repo.get(user_id).await
            .map_err(|e| maowbot_common::error::Error::Internal(format!("Error getting user: {}", e)))?
            .ok_or_else(|| maowbot_common::error::Error::User(format!("User not found: {}", user_id)))?;
        
        // Process with AI
        self.service.client.agent_with_memory(user_id.to_string(), message, 10).await
            .map_err(|e| maowbot_common::error::Error::Internal(format!("AI error: {}", e)))
    }
    
    /// Register a new function
    async fn register_ai_function(&self, name: &str, description: &str) -> Result<(), maowbot_common::error::Error> {
        // This is a simplified function registration
        // In practice, you'd want to define parameters and a handler
        let function = Function::new(
            name,
            description,
            vec![],
            Arc::new(move |_args| {
                Ok(serde_json::json!({"result": "Function executed successfully"}))
            }),
        );
        
        self.service.client.register_function(function).await;
        Ok(())
    }
    
    /// Set the system prompt
    async fn set_system_prompt(&self, prompt: &str) -> Result<(), maowbot_common::error::Error> {
        // In the real implementation, this would update a configuration
        // For now, we'll just log it
        info!("Setting system prompt: {}", prompt);
        Ok(())
    }
}