use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use reqwest::Client;
use serde_json::json;
use tokio::sync::RwLock;

use crate::function::FunctionSchema;
use crate::models::ProviderConfig;
use crate::traits::{ChatMessage, ChatResponse, FunctionCall, ModelProvider};

/// OpenAI provider implementation
pub struct OpenAIProvider {
    config: ProviderConfig,
    client: Client,
}

impl OpenAIProvider {
    /// Create a new OpenAI provider with the given configuration
    pub fn new(config: ProviderConfig) -> Self {
        let client = Client::new();
        Self { config, client }
    }
}

#[async_trait]
impl ModelProvider for OpenAIProvider {
    fn name(&self) -> &str {
        "openai"
    }
    
    async fn complete(&self, prompt: &str) -> anyhow::Result<String> {
        let api_base = self.config.api_base.clone().unwrap_or_else(|| {
            "https://api.openai.com/v1".to_string()
        });
        
        let response = self.client
            .post(format!("{}/completions", api_base))
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .json(&json!({
                "model": self.config.default_model,
                "prompt": prompt,
                "max_tokens": 1000,
                "temperature": 0.7,
            }))
            .send()
            .await?;
        
        let data = response.json::<serde_json::Value>().await?;
        
        let choices = data["choices"].as_array()
            .ok_or_else(|| anyhow::anyhow!("Invalid response format"))?;
        
        if choices.is_empty() {
            return Err(anyhow::anyhow!("No completions returned"));
        }
        
        let text = choices[0]["text"].as_str()
            .ok_or_else(|| anyhow::anyhow!("Invalid response format"))?
            .to_string();
        
        Ok(text)
    }
    
    async fn chat(&self, messages: Vec<ChatMessage>) -> anyhow::Result<String> {
        let api_base = self.config.api_base.clone().unwrap_or_else(|| {
            "https://api.openai.com/v1".to_string()
        });
        
        // Check if there's already a system message in the messages
        let has_system_message = messages.iter().any(|msg| msg.role == "system");
        
        // Create a vector to hold all messages, potentially with a system message added
        let mut all_messages = Vec::new();
        
        // Add system message from config if not already present
        if !has_system_message {
            // Check if there's a system_prompt in the options
            if let Some(system_prompt) = self.config.options.get("system_prompt") {
                all_messages.push(json!({
                    "role": "system",
                    "content": system_prompt
                }));
            }
        }
        
        // Add the user messages
        let mut formatted_messages: Vec<serde_json::Value> = messages
            .iter()
            .map(|msg| {
                json!({
                    "role": msg.role,
                    "content": msg.content
                })
            })
            .collect();
        
        // Combine system message (if added) with other messages
        all_messages.append(&mut formatted_messages);
        
        // Build the request payload - simpler version with only required fields
        let mut request_payload = json!({
            "model": self.config.default_model,
            "messages": all_messages,
            "max_tokens": 1000,
        });
        
        // Check if we should use web search
        let use_web_search = self.config.options.get("enable_web_search").map_or(false, |v| v == "true") ||
                            self.config.default_model == "gpt-4o-search-preview" || 
                            self.config.default_model == "gpt-4o" ||
                            self.config.default_model == "gpt-4.1";
                            
        tracing::info!("Web search check: enable_web_search={}, model={}", 
                     self.config.options.get("enable_web_search").map_or("false", |v| v), 
                     self.config.default_model);
        
        if use_web_search {
            tracing::info!("Using web search capabilities with gpt-4o-search-preview");
            
            // Set model to gpt-4o-search-preview explicitly
            request_payload["model"] = json!("gpt-4o-search-preview");
            
            // IMPORTANT: This is the ONLY parameter needed for web search - no others required
            request_payload["web_search_options"] = json!({
                      "search_context_size": "medium"
            });
            
            tracing::info!("Web search enabled: model={}, web_search=true", 
                         request_payload["model"].as_str().unwrap_or("unknown"));
        } else {
            tracing::info!("Standard chat completion without web search");
        }
        // Request to OpenAI API
        // Convert request payload to pretty-printed JSON for logging
        let payload_json = serde_json::to_string_pretty(&request_payload)
            .unwrap_or_else(|_| format!("{:?}", request_payload));
        
        // Log the final API call details
        tracing::info!("Making API call to {}/chat/completions", api_base);
        tracing::info!("Final API request payload:\n{}", payload_json);
        
        // Send the request
        let response = self.client
            .post(format!("{}/chat/completions", api_base))
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .json(&request_payload)
            .send()
            .await?;
        
        // Get the raw response text first for better error handling
        let response_text = response.text().await?;
        tracing::debug!("Raw API response: {}", response_text);
        
        // Try to parse as JSON
        let data = match serde_json::from_str::<serde_json::Value>(&response_text) {
            Ok(json) => json,
            Err(e) => {
                tracing::error!("Failed to parse API response as JSON: {:?}", e);
                tracing::error!("Response text: {}", response_text);
                return Err(anyhow::anyhow!("API returned non-JSON response: {}", e));
            }
        };
        
        // Check for API errors
        if let Some(error) = data.get("error") {
            tracing::error!("API returned error: {:?}", error);
            let error_message = error.get("message").and_then(|m| m.as_str()).unwrap_or("Unknown error");
            return Err(anyhow::anyhow!("API error: {}", error_message));
        }
        
        // Extract choices
        let choices = match data.get("choices").and_then(|c| c.as_array()) {
            Some(choices) => choices,
            None => {
                tracing::error!("Response missing 'choices' array: {:?}", data);
                return Err(anyhow::anyhow!("Response missing 'choices' array"));
            }
        };
        
        if choices.is_empty() {
            tracing::error!("API returned empty choices array");
            return Err(anyhow::anyhow!("No completions returned"));
        }
        
        // Extract content from first choice
        let message = &choices[0].get("message").ok_or_else(|| {
            tracing::error!("First choice missing 'message': {:?}", choices[0]);
            anyhow::anyhow!("Response choice missing 'message'")
        })?;
        
        let content = message.get("content").and_then(|c| c.as_str()).ok_or_else(|| {
            tracing::error!("Message missing 'content': {:?}", message);
            anyhow::anyhow!("Response message missing 'content'")
        })?.to_string();
        
        Ok(content)
    }
    
    async fn chat_with_functions(
        &self, 
        messages: Vec<ChatMessage>,
        functions: Vec<crate::function::Function>
    ) -> anyhow::Result<ChatResponse> {
        let api_base = self.config.api_base.clone().unwrap_or_else(|| {
            "https://api.openai.com/v1".to_string()
        });
        
        // Check if there's already a system message in the messages
        let has_system_message = messages.iter().any(|msg| msg.role == "system");
        
        // Create a vector to hold all messages, potentially with a system message added
        let mut all_messages = Vec::new();
        
        // Add system message from config if not already present
        if !has_system_message {
            // Check if there's a system_prompt in the options
            if let Some(system_prompt) = self.config.options.get("system_prompt") {
                all_messages.push(json!({
                    "role": "system",
                    "content": system_prompt
                }));
            }
        }
        
        // Add the user messages
        let mut formatted_messages: Vec<serde_json::Value> = messages
            .iter()
            .map(|msg| {
                json!({
                    "role": msg.role,
                    "content": msg.content
                })
            })
            .collect();
            
        // Combine system message (if added) with other messages
        all_messages.append(&mut formatted_messages);
        
        let function_schemas: Vec<FunctionSchema> = functions
            .iter()
            .map(|f| f.to_schema())
            .collect();
        
        let formatted_functions: Vec<serde_json::Value> = function_schemas
            .iter()
            .map(|schema| {
                json!({
                    "name": schema.name,
                    "description": schema.description,
                    "parameters": schema.parameters,
                })
            })
            .collect();
        
        // Build request payload
        let request_payload = json!({
            "model": self.config.default_model,
            "messages": all_messages,
            "functions": formatted_functions,
            "function_call": "auto",
            "max_tokens": 1000
        });
        
        // Convert request payload to pretty-printed JSON for logging
        let payload_json = serde_json::to_string_pretty(&request_payload)
            .unwrap_or_else(|_| format!("{:?}", request_payload));
        
        // Log the final API call details
        tracing::info!("Making API call to {}/chat/completions for function calling", api_base);
        tracing::info!("Final API request payload:\n{}", payload_json);
        
        // Send the request
        let response = self.client
            .post(format!("{}/chat/completions", api_base))
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .json(&request_payload)
            .send()
            .await?;
        
        // Get the raw response text first for better error handling
        let response_text = response.text().await?;
        tracing::debug!("Raw API response: {}", response_text);
        
        // Try to parse as JSON
        let data = match serde_json::from_str::<serde_json::Value>(&response_text) {
            Ok(json) => json,
            Err(e) => {
                tracing::error!("Failed to parse API response as JSON: {:?}", e);
                tracing::error!("Response text: {}", response_text);
                return Err(anyhow::anyhow!("API returned non-JSON response: {}", e));
            }
        };
        
        // Check for API errors
        if let Some(error) = data.get("error") {
            tracing::error!("API returned error: {:?}", error);
            let error_message = error.get("message").and_then(|m| m.as_str()).unwrap_or("Unknown error");
            return Err(anyhow::anyhow!("API error: {}", error_message));
        }
        
        // Extract choices
        let choices = match data.get("choices").and_then(|c| c.as_array()) {
            Some(choices) => choices,
            None => {
                tracing::error!("Response missing 'choices' array: {:?}", data);
                return Err(anyhow::anyhow!("Response missing 'choices' array"));
            }
        };
        
        if choices.is_empty() {
            tracing::error!("API returned empty choices array");
            return Err(anyhow::anyhow!("No completions returned"));
        }
        
        // Extract content and function call
        let message = match choices[0].get("message") {
            Some(msg) => msg,
            None => {
                tracing::error!("First choice missing 'message': {:?}", choices[0]);
                return Err(anyhow::anyhow!("Response choice missing 'message'"));
            }
        };
        
        let content = message.get("content").and_then(|c| c.as_str()).map(|s| s.to_string());
        
        let function_call = match message.get("function_call") {
            Some(fc) if fc.is_object() => {
                // Extract function name
                let name = match fc.get("name").and_then(|n| n.as_str()) {
                    Some(name) => name.to_string(),
                    None => {
                        tracing::error!("Function call missing 'name': {:?}", fc);
                        return Err(anyhow::anyhow!("Invalid function call format - missing name"));
                    }
                };
                
                // Extract arguments
                let arguments_str = match fc.get("arguments").and_then(|a| a.as_str()) {
                    Some(args) => args,
                    None => {
                        tracing::error!("Function call missing 'arguments': {:?}", fc);
                        return Err(anyhow::anyhow!("Invalid function call format - missing arguments"));
                    }
                };
                
                // Parse arguments JSON
                let arguments_value = match serde_json::from_str::<serde_json::Value>(arguments_str) {
                    Ok(value) => value,
                    Err(e) => {
                        tracing::error!("Failed to parse function arguments JSON: {:?}", e);
                        tracing::error!("Arguments string: {}", arguments_str);
                        return Err(anyhow::anyhow!("Invalid function arguments JSON: {}", e));
                    }
                };
                
                // Build arguments map
                let mut arguments = HashMap::new();
                if let Some(obj) = arguments_value.as_object() {
                    for (key, value) in obj {
                        arguments.insert(key.clone(), value.clone());
                    }
                }
                
                Some(FunctionCall { name, arguments })
            },
            _ => None
        };
        
        Ok(ChatResponse { content, function_call })
    }
}

/// Anthropic provider implementation
pub struct AnthropicProvider {
    config: ProviderConfig,
    client: Client,
}

impl AnthropicProvider {
    /// Create a new Anthropic provider with the given configuration
    pub fn new(config: ProviderConfig) -> Self {
        let client = Client::new();
        Self { config, client }
    }
}

#[async_trait]
impl ModelProvider for AnthropicProvider {
    fn name(&self) -> &str {
        "anthropic"
    }
    
    async fn complete(&self, prompt: &str) -> anyhow::Result<String> {
        let api_base = self.config.api_base.clone().unwrap_or_else(|| {
            "https://api.anthropic.com/v1".to_string()
        });
        
        let response = self.client
            .post(format!("{}/complete", api_base))
            .header("x-api-key", &self.config.api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&json!({
                "model": self.config.default_model,
                "prompt": format!("\n\nHuman: {}\n\nAssistant:", prompt),
                "max_tokens_to_sample": 1000,
                "temperature": 0.7,
            }))
            .send()
            .await?;
        
        let data = response.json::<serde_json::Value>().await?;
        let completion = data["completion"].as_str()
            .ok_or_else(|| anyhow::anyhow!("Invalid response format"))?
            .to_string();
        
        Ok(completion)
    }
    
    async fn chat(&self, messages: Vec<ChatMessage>) -> anyhow::Result<String> {
        // For older Claude models, convert chat format to Anthropic's format
        let mut prompt = String::new();
        
        for message in &messages {
            let role = match message.role.as_str() {
                "user" => "Human",
                "assistant" => "Assistant",
                "system" => continue, // Handle system messages specially
                _ => continue,
            };
            
            prompt.push_str(&format!("\n\n{}: {}", role, message.content));
        }
        
        // Add final Assistant prompt 
        prompt.push_str("\n\nAssistant:");
        
        // Extract system message if present
        let system_message = messages.iter()
            .find(|msg| msg.role == "system")
            .map(|msg| msg.content.clone());
        
        let api_base = self.config.api_base.clone().unwrap_or_else(|| {
            "https://api.anthropic.com/v1".to_string()
        });
        
        let mut request = json!({
            "model": self.config.default_model,
            "prompt": prompt,
            "max_tokens_to_sample": 1000,
            "temperature": 0.7,
        });
        
        if let Some(system) = system_message {
            request["system"] = json!(system);
        }
        
        let response = self.client
            .post(format!("{}/complete", api_base))
            .header("x-api-key", &self.config.api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&request)
            .send()
            .await?;
        
        let data = response.json::<serde_json::Value>().await?;
        let completion = data["completion"].as_str()
            .ok_or_else(|| anyhow::anyhow!("Invalid response format"))?
            .to_string();
        
        Ok(completion)
    }
    
    async fn chat_with_functions(
        &self, 
        messages: Vec<ChatMessage>,
        functions: Vec<crate::function::Function>
    ) -> anyhow::Result<ChatResponse> {
        // Newer Claude API with function calling
        let api_base = self.config.api_base.clone().unwrap_or_else(|| {
            "https://api.anthropic.com/v1".to_string()
        });
        
        let formatted_messages: Vec<serde_json::Value> = messages
            .iter()
            .map(|msg| {
                let role = match msg.role.as_str() {
                    "user" => "user",
                    "assistant" => "assistant",
                    "system" => "system",
                    _ => "user", // Default to user for unknown roles
                };
                
                json!({
                    "role": role,
                    "content": msg.content
                })
            })
            .collect();
        
        let function_schemas: Vec<FunctionSchema> = functions
            .iter()
            .map(|f| f.to_schema())
            .collect();
        
        let formatted_tools: Vec<serde_json::Value> = function_schemas
            .iter()
            .map(|schema| {
                json!({
                    "name": schema.name,
                    "description": schema.description,
                    "parameters": schema.parameters,
                })
            })
            .collect();
        
        let response = self.client
            .post(format!("{}/messages", api_base))  // Using messages API
            .header("x-api-key", &self.config.api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&json!({
                "model": self.config.default_model,
                "messages": formatted_messages,
                "tools": [{
                    "type": "function",
                    "functions": formatted_tools
                }],
                "max_tokens": 1000,
                "temperature": 0.7,
            }))
            .send()
            .await?;
        
        let data = response.json::<serde_json::Value>().await?;
        
        let content = data["content"].as_array()
            .ok_or_else(|| anyhow::anyhow!("Invalid response format"))?;
        
        // Extract text or function call from response
        let mut text_content = None;
        let mut function_call = None;
        
        for item in content {
            let item_type = item["type"].as_str().unwrap_or("");
            
            if item_type == "text" {
                text_content = item["text"].as_str().map(|s| s.to_string());
            } else if item_type == "tool_use" {
                let tool_use = &item["tool_use"];
                if tool_use["type"].as_str() == Some("function") {
                    let name = tool_use["name"].as_str()
                        .ok_or_else(|| anyhow::anyhow!("Invalid function call format"))?
                        .to_string();
                    
                    let args = &tool_use["parameters"];
                    
                    let mut arguments = HashMap::new();
                    if let Some(obj) = args.as_object() {
                        for (key, value) in obj {
                            arguments.insert(key.clone(), value.clone());
                        }
                    }
                    
                    function_call = Some(FunctionCall { name, arguments });
                }
            }
        }
        
        Ok(ChatResponse { content: text_content, function_call })
    }
}

/// Factory for creating AI providers
pub struct Provider {
    providers: Arc<RwLock<HashMap<String, Arc<dyn ModelProvider>>>>,
}

impl Provider {
    /// Create a new provider factory
    pub fn new() -> Self {
        Self {
            providers: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    /// Register a new provider
    pub async fn register<P: ModelProvider + 'static>(&self, provider: P) {
        let mut providers = self.providers.write().await;
        providers.insert(provider.name().to_string(), Arc::new(provider));
    }
    
    /// Get a provider by name
    pub async fn get(&self, name: &str) -> Option<Arc<dyn ModelProvider>> {
        let providers = self.providers.read().await;
        providers.get(name).cloned()
    }
    
    /// Get all registered providers
    pub async fn get_all(&self) -> Vec<String> {
        let providers = self.providers.read().await;
        providers.keys().cloned().collect()
    }
    
    /// Create an OpenAI provider from a configuration
    pub fn create_openai(config: ProviderConfig) -> OpenAIProvider {
        OpenAIProvider::new(config)
    }
    
    /// Create an Anthropic provider from a configuration
    pub fn create_anthropic(config: ProviderConfig) -> AnthropicProvider {
        AnthropicProvider::new(config)
    }
}
