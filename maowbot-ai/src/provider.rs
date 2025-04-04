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
        
        let formatted_messages: Vec<serde_json::Value> = messages
            .iter()
            .map(|msg| {
                json!({
                    "role": msg.role,
                    "content": msg.content
                })
            })
            .collect();
        
        let response = self.client
            .post(format!("{}/chat/completions", api_base))
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .json(&json!({
                "model": self.config.default_model,
                "messages": formatted_messages,
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
        
        let content = choices[0]["message"]["content"].as_str()
            .ok_or_else(|| anyhow::anyhow!("Invalid response format"))?
            .to_string();
        
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
        
        let formatted_messages: Vec<serde_json::Value> = messages
            .iter()
            .map(|msg| {
                json!({
                    "role": msg.role,
                    "content": msg.content
                })
            })
            .collect();
        
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
        
        let response = self.client
            .post(format!("{}/chat/completions", api_base))
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .json(&json!({
                "model": self.config.default_model,
                "messages": formatted_messages,
                "functions": formatted_functions,
                "function_call": "auto",
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
        
        let message = &choices[0]["message"];
        let content = message["content"].as_str().map(|s| s.to_string());
        
        let function_call = if message["function_call"].is_object() {
            let function_call_obj = &message["function_call"];
            let name = function_call_obj["name"].as_str()
                .ok_or_else(|| anyhow::anyhow!("Invalid function call format"))?
                .to_string();
            
            let arguments_str = function_call_obj["arguments"].as_str()
                .ok_or_else(|| anyhow::anyhow!("Invalid function call arguments"))?;
            
            let arguments_value: serde_json::Value = serde_json::from_str(arguments_str)?;
            
            let mut arguments = HashMap::new();
            if let Some(obj) = arguments_value.as_object() {
                for (key, value) in obj {
                    arguments.insert(key.clone(), value.clone());
                }
            }
            
            Some(FunctionCall { name, arguments })
        } else {
            None
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