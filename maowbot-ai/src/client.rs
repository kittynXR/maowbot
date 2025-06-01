use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::RwLock;
use tracing::{debug, error, info, trace};
use uuid::Uuid;

use crate::function::{Function, FunctionRegistry};
use crate::memory::MemoryManager;
use crate::provider::Provider;
use crate::traits::{AiApi, ChatMessage, ChatResponse, ModelProvider};

/// Represents a client for AI services
pub struct AiClient {
    /// Provider registry for different AI models
    provider: Arc<Provider>,
    
    /// Memory manager for conversation history
    memory: Arc<MemoryManager>,
    
    /// Function registry for callable functions
    functions: Arc<FunctionRegistry>,
    
    /// Default provider to use
    default_provider: String,
}

impl AiClient {
    /// Create a new AI client with the given components
    pub fn new(
        provider: Arc<Provider>,
        memory: Arc<MemoryManager>,
        functions: Arc<FunctionRegistry>,
        default_provider: impl Into<String>,
    ) -> Self {
        Self {
            provider,
            memory,
            functions,
            default_provider: default_provider.into(),
        }
    }
    
    /// Set the default provider
    pub fn set_default_provider(&mut self, provider: impl Into<String>) {
        self.default_provider = provider.into();
    }
    
    /// Get the provider registry
    pub fn provider(&self) -> Arc<Provider> {
        self.provider.clone()
    }
    
    /// Get the memory manager
    pub fn memory(&self) -> Arc<MemoryManager> {
        self.memory.clone()
    }
    
    /// Get the function registry
    pub fn functions(&self) -> Arc<FunctionRegistry> {
        self.functions.clone()
    }
    
    /// Simple completion with just a prompt
    pub async fn complete(&self, prompt: &str) -> anyhow::Result<String> {
        let provider = self.get_provider(None).await?;
        provider.complete(prompt).await
    }
    
    /// Chat completion with context
    pub async fn chat(&self, messages: Vec<ChatMessage>) -> anyhow::Result<String> {
        let provider = self.get_provider(None).await?;
        provider.chat(messages).await
    }
    
    /// Chat with user context from memory
    pub async fn chat_with_user_context(
        &self,
        user_id: impl Into<String>,
        message: &str,
        context_size: usize,
    ) -> anyhow::Result<String> {
        let user_id = user_id.into();
        
        // Store user message
        self.memory.store_message(
            &user_id,
            ChatMessage {
                role: "user".to_string(),
                content: message.to_string(),
            },
        ).await?;
        
        // Retrieve conversation history
        let mut messages = self.memory.retrieve_messages(&user_id, context_size).await?;
        
        // Add a system message at the beginning if not present
        if !messages.iter().any(|msg| msg.role == "system") {
            messages.insert(0, ChatMessage {
                role: "system".to_string(),
                content: "You are a helpful AI assistant for MaowBot.".to_string(),
            });
        }
        
        // Get response
        let provider = self.get_provider(None).await?;
        let response = provider.chat(messages.clone()).await?;
        
        // Store assistant's response
        self.memory.store_message(
            &user_id,
            ChatMessage {
                role: "assistant".to_string(),
                content: response.clone(),
            },
        ).await?;
        
        Ok(response)
    }

    pub async fn chat_with_search(
        &self,
        messages: Vec<ChatMessage>,
    ) -> anyhow::Result<serde_json::Value> {
        let provider = self.get_provider(None).await?;
        provider.chat_with_search(messages).await
    }

    /// Chat with function calling capabilities
    pub async fn chat_with_functions(
        &self,
        messages: Vec<ChatMessage>,
        function_names: Option<Vec<String>>,
    ) -> anyhow::Result<ChatResponse> {
        let provider = self.get_provider(None).await?;
        
        // Get all functions or filter by name
        let functions = match function_names {
            Some(names) => {
                let mut functions = Vec::new();
                for name in names {
                    if let Some(function) = self.functions.get(&name).await {
                        functions.push(function);
                    }
                }
                functions
            },
            None => self.functions.get_all().await,
        };
        
        provider.chat_with_functions(messages, functions).await
    }
    
    /// Execute an agent flow with function calling and memory
    pub async fn agent_with_memory(
        &self,
        user_id: impl Into<String>,
        message: &str,
        context_size: usize,
    ) -> anyhow::Result<String> {
        let user_id = user_id.into();
        
        // Store user message
        self.memory.store_message(
            &user_id,
            ChatMessage {
                role: "user".to_string(),
                content: message.to_string(),
            },
        ).await?;
        
        // Retrieve conversation history
        let mut messages = self.memory.retrieve_messages(&user_id, context_size).await?;
        
        // Add a system message at the beginning if not present
        if !messages.iter().any(|msg| msg.role == "system") {
            messages.insert(0, ChatMessage {
                role: "system".to_string(),
                content: "You are a helpful AI assistant for MaowBot with access to functions. When appropriate, call functions to complete tasks for the user.".to_string(),
            });
        }
        
        let provider = self.get_provider(None).await?;
        let functions = self.functions.get_all().await;
        
        // Get initial response
        let response = provider.chat_with_functions(messages.clone(), functions).await?;
        
        // Handle function call if present
        let final_response = if let Some(function_call) = response.function_call {
            debug!("Function call requested: {}", function_call.name);
            
            // Execute the function
            let result = match self.functions.execute(&function_call.name, function_call.arguments).await {
                Ok(result) => {
                    let result_str = result.to_string();
                    
                    // Store function call in memory
                    self.memory.store_message(
                        &user_id,
                        ChatMessage {
                            role: "assistant".to_string(),
                            content: format!("I'm calling the {} function.", function_call.name),
                        },
                    ).await?;
                    
                    // Store function result in memory
                    self.memory.store_message(
                        &user_id,
                        ChatMessage {
                            role: "function".to_string(),
                            content: format!("Result from {}: {}", function_call.name, result_str),
                        },
                    ).await?;
                    
                    // Get updated messages with function result
                    let updated_messages = self.memory.retrieve_messages(&user_id, context_size + 2).await?;
                    
                    // Get a new response with the function result
                    let followup_response = provider.chat(updated_messages).await?;
                    
                    // Store final assistant response
                    self.memory.store_message(
                        &user_id,
                        ChatMessage {
                            role: "assistant".to_string(),
                            content: followup_response.clone(),
                        },
                    ).await?;
                    
                    followup_response
                },
                Err(err) => {
                    error!("Error executing function {}: {}", function_call.name, err);
                    
                    // Store error in memory
                    self.memory.store_message(
                        &user_id,
                        ChatMessage {
                            role: "function".to_string(),
                            content: format!("Error calling {}: {}", function_call.name, err),
                        },
                    ).await?;
                    
                    // Get updated messages with error
                    let updated_messages = self.memory.retrieve_messages(&user_id, context_size + 2).await?;
                    
                    // Get a new response with the error
                    let error_response = provider.chat(updated_messages).await?;
                    
                    // Store final assistant response
                    self.memory.store_message(
                        &user_id,
                        ChatMessage {
                            role: "assistant".to_string(),
                            content: error_response.clone(),
                        },
                    ).await?;
                    
                    error_response
                }
            };
            
            result
        } else {
            // If no function call, use the text response
            let text_response = response.content.unwrap_or_else(|| "I don't know how to respond.".to_string());
            
            // Store assistant's response
            self.memory.store_message(
                &user_id,
                ChatMessage {
                    role: "assistant".to_string(),
                    content: text_response.clone(),
                },
            ).await?;
            
            text_response
        };
        
        Ok(final_response)
    }
    
    /// Register a new function in the registry
    pub async fn register_function(&self, function: Function) {
        self.functions.register(function).await;
    }
    
    /// Get a provider by name or the default provider
    async fn get_provider(&self, name: Option<&str>) -> anyhow::Result<Arc<dyn ModelProvider>> {
        if let Some(provider_name) = name {
            // If a specific provider is requested, use that
            return self.provider.get(provider_name).await
                .ok_or_else(|| anyhow::anyhow!("Provider not found: {}", provider_name));
        }
        
        // If no provider specified, try the default provider
        if let Some(provider) = self.provider.get(&self.default_provider).await {
            return Ok(provider);
        }
        
        // If default provider not found, try to get the first available provider
        let providers = self.provider.get_all().await;
        if providers.is_empty() {
            return Err(anyhow::anyhow!("No AI providers configured"));
        }
        
        // Use the first provider in the list
        let first_provider = &providers[0];
        self.provider.get(first_provider).await
            .ok_or_else(|| anyhow::anyhow!("Provider not found: {}", first_provider))
    }
}

/// Implementation of the BotApi AiApi trait
pub struct MaowBotAiApi {
    client: Arc<AiClient>,
    system_prompt: RwLock<String>,
}

impl MaowBotAiApi {
    /// Create a new MaowBotAiApi with the given AiClient
    pub fn new(client: Arc<AiClient>) -> Self {
        Self {
            client,
            system_prompt: RwLock::new("You are a helpful AI assistant for MaowBot.".to_string()),
        }
    }
}

#[async_trait]
impl maowbot_common::traits::api::AiApi for MaowBotAiApi {
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
            
        self.client.chat(chat_messages).await
            .map_err(|e| maowbot_common::error::Error::Internal(format!("AI error: {}", e)))
    }

    async fn generate_with_search(
        &self,
        messages: Vec<serde_json::Value>,
    ) -> Result<serde_json::Value, maowbot_common::error::Error> {
        use crate::traits::ChatMessage;           // correct local path

        // JSON → ChatMessage
        let chat_messages = messages
            .into_iter()
            .filter_map(|m| {
                Some(ChatMessage {
                    role:    m.get("role")?.as_str()?.to_string(),
                    content: m.get("content")?.as_str()?.to_string(),
                })
            })
            .collect::<Vec<_>>();

        // Delegate to the AiClient helper we added earlier
        self.client
            .chat_with_search(chat_messages)
            .await
            .map_err(|e| maowbot_common::error::Error::Internal(format!("AI error: {e}")))
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
            
        let response = self.client.chat_with_functions(chat_messages, None).await
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
        self.client.agent_with_memory(user_id.to_string(), message, 10).await
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
        
        self.client.register_function(function).await;
        Ok(())
    }
    
    /// Set the system prompt
    async fn set_system_prompt(&self, prompt: &str) -> Result<(), maowbot_common::error::Error> {
        let mut system_prompt = self.system_prompt.write().await;
        *system_prompt = prompt.to_string();
        Ok(())
    }
}