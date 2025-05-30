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
        let api_base = self
            .config
            .api_base
            .clone()
            .unwrap_or_else(|| "https://api.openai.com/v1".to_string());

        /* ---------- 1. build message array (unchanged) ---------- */
        let has_system_message = messages.iter().any(|m| m.role == "system");
        let mut all_messages = Vec::new();

        if !has_system_message {
            if let Some(sys) = self.config.options.get("system_prompt") {
                all_messages.push(json!({ "role": "system", "content": sys }));
            }
        }

        let mut formatted: Vec<serde_json::Value> = messages
            .iter()
            .map(|m| json!({ "role": m.role, "content": m.content }))
            .collect();

        all_messages.append(&mut formatted);

        /* ---------- 2. decide whether to enable web-search ---------- */
        // - enable if the *caller* asked for it in options, OR
        // - the model name itself already includes "-search" (e.g. gpt-4o-search-preview)
        let opt_flag = self
            .config
            .options
            .get("enable_web_search")
            .map(|v| v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);

        let model_contains_search = self
            .config
            .default_model
            .to_lowercase()
            .contains("-search");

        let use_web_search = opt_flag || model_contains_search;

        /* ---------- 3. prepare request payload ---------- */
        let mut payload = json!({
            "model": self.config.default_model,
            "messages": all_messages,
            "max_tokens": 1000
        });

        if use_web_search {
            // If the caller supplied a normal model (e.g. gpt-4o) **and**
            // asked for web search via `enable_web_search=true`,
            // transparently upgrade it to the matching search-preview variant.
            if !model_contains_search {
                payload["model"] = json!("gpt-4o-search-preview");
            }

            payload["web_search_options"] = json!({ "search_context_size": "medium" });
            tracing::info!(
                "🔍 Web search enabled → model={}",
                payload["model"].as_str().unwrap_or("unknown")
            );
        } else {
            tracing::info!("Standard chat completion (no web search) → model={}", self.config.default_model);
        }

        /* ---------- 4. fire request & handle response (unchanged) ---------- */
        let payload_json = serde_json::to_string_pretty(&payload)
            .unwrap_or_else(|_| format!("{:?}", payload));
        tracing::info!("Making API call to {}/chat/completions\n{}", api_base, payload_json);

        let response = self
            .client
            .post(format!("{}/chat/completions", api_base))
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .json(&payload)
            .send()
            .await?;

        let raw = response.text().await?;
        tracing::debug!("Raw API response: {}", raw);

        let data: serde_json::Value = serde_json::from_str(&raw)
            .map_err(|e| anyhow::anyhow!("API returned non-JSON: {}", e))?;

        if let Some(err) = data.get("error") {
            let msg = err
                .get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("unknown error");
            return Err(anyhow::anyhow!("API error: {}", msg));
        }

        let choices = data["choices"]
            .as_array()
            .ok_or_else(|| anyhow::anyhow!("Missing choices"))?;
        if choices.is_empty() {
            return Err(anyhow::anyhow!("No completions returned"));
        }

        let content = choices[0]["message"]["content"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing content"))?
            .to_owned();

        Ok(content)
    }

    async fn chat_with_search(
        &self,
        messages: Vec<ChatMessage>,
    ) -> anyhow::Result<serde_json::Value> {
        /* ----------------  build full payload  ---------------- */
        let api_base = self
            .config
            .api_base
            .clone()
            .unwrap_or_else(|| "https://api.openai.com/v1".into());

        let mut all = Vec::new();
        if !messages.iter().any(|m| m.role == "system") {
            if let Some(sys) = self.config.options.get("system_prompt") {
                all.push(json!({ "role": "system", "content": sys }));
            }
        }
        all.extend(messages.iter().map(|m| json!({ "role": m.role, "content": m.content })));

        let model = if self
            .config
            .default_model
            .to_lowercase()
            .contains("-search")
        {
            self.config.default_model.clone()
        } else {
            "gpt-4o-search-preview".into()
        };

        let payload = json!({
            "model": model,
            "messages": all,
            "max_tokens": 1000,
            "web_search_options": { "search_context_size": "medium" }
        });

        let raw = self
            .client
            .post(format!("{}/chat/completions", api_base))
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .json(&payload)
            .send()
            .await?
            .text()
            .await?;

        let data: serde_json::Value = serde_json::from_str(&raw)
            .map_err(|e| anyhow::anyhow!("API returned non‑JSON: {}", e))?;

        /* ----------------  pull content & annotations  ---------------- */
        let msg   = &data["choices"][0]["message"];
        let text  = msg["content"].as_str().unwrap_or("").to_owned();
        let anns  = msg["annotations"].as_array().cloned().unwrap_or_default();

        /* ---- strip URLs from the text & leave ‘*’ markers ---- */
        // collect ranges first, then rebuild string to avoid shifting indices
        let mut ranges: Vec<(usize, usize, String, String)> = anns
            .iter()
            .filter_map(|a| {
                let cite = &a["url_citation"];
                Some((
                    cite["start_index"].as_u64()? as usize,
                    cite["end_index"].as_u64()? as usize,
                    cite["title"].as_str()?.to_owned(),
                    cite["url"].as_str()?.to_owned(),
                ))
            })
            .collect();

        ranges.sort_by_key(|r| r.0);               // ascending
        
        // Remove overlapping ranges and ensure indices are valid
        let mut filtered_ranges = Vec::new();
        let mut last_end = 0;
        
        for (start, end, title, url) in ranges {
            // Skip invalid ranges
            if start >= end || start >= text.len() || end > text.len() {
                tracing::warn!("Skipping invalid annotation range: start={}, end={}, text_len={}", start, end, text.len());
                continue;
            }
            
            // Skip overlapping ranges
            if start < last_end {
                tracing::warn!("Skipping overlapping annotation: start={}, last_end={}", start, last_end);
                continue;
            }
            
            filtered_ranges.push((start, end, title, url));
            last_end = end;
        }
        
        // Build cleaned text
        let mut cleaned = String::new();
        let mut last = 0usize;
        
        for (s, e, ..) in &filtered_ranges {
            // Ensure we're not going out of bounds
            if last <= *s && *s <= text.len() {
                cleaned.push_str(&text[last..*s]);
                cleaned.push('*');
                last = *e;
            }
        }
        
        // Add remaining text if any
        if last < text.len() {
            cleaned.push_str(&text[last..]);
        }

        // build a friendlier sources array
        let sources: Vec<serde_json::Value> = filtered_ranges
            .into_iter()
            .map(|(_, _, title, url)| json!({ "title": title, "url": url }))
            .collect();

        Ok(json!({
            "content":     cleaned.trim(),
            "annotations": anns,      // keep raw for backward compat
            "sources":     sources
        }))
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

    async fn chat_with_search(&self, messages: Vec<ChatMessage>)
                              -> anyhow::Result<serde_json::Value> {
        // naïve fallback: just run normal chat
        Ok(serde_json::json!({ "content": self.chat(messages).await?, "annotations": [], "sources": [] }))
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
