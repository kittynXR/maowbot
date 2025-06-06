use std::collections::HashMap;
use anyhow::Result;
use maowbot_proto::maowbot::services::*;
use maowbot_proto::prost_types;
use crate::grpc_client::GrpcClient;
use serde_json::json;

/// AI command handler for common UI functionality
pub struct AiCommands;

impl AiCommands {
    /// Enable the AI service
    pub async fn enable(client: &mut GrpcClient) -> Result<String> {
        let response = client.ai
            .enable_ai(EnableAiRequest {})
            .await?
            .into_inner();
        
        if response.success {
            Ok(response.message)
        } else {
            Err(anyhow::anyhow!(response.message))
        }
    }
    
    /// Disable the AI service
    pub async fn disable(client: &mut GrpcClient) -> Result<String> {
        let response = client.ai
            .disable_ai(DisableAiRequest {})
            .await?
            .into_inner();
        
        if response.success {
            Ok(response.message)
        } else {
            Err(anyhow::anyhow!(response.message))
        }
    }
    
    /// Get AI service status
    pub async fn status(client: &mut GrpcClient) -> Result<AiStatusInfo> {
        let response = client.ai
            .get_ai_status(GetAiStatusRequest {})
            .await?
            .into_inner();
        
        Ok(AiStatusInfo {
            enabled: response.enabled,
            active_provider: response.active_provider,
            active_models_count: response.active_models_count,
            active_agents_count: response.active_agents_count,
            statistics: response.statistics,
        })
    }
    
    /// Show provider API keys (masked)
    pub async fn show_provider_keys(client: &mut GrpcClient, provider_name: Option<String>) -> Result<Vec<ProviderKeyDisplay>> {
        let response = client.ai
            .show_provider_keys(ShowProviderKeysRequest {
                provider_name: provider_name.unwrap_or_default(),
            })
            .await?
            .into_inner();
        
        Ok(response.keys.into_iter().map(|key| ProviderKeyDisplay {
            provider_name: key.provider_name,
            masked_key: key.masked_key,
            api_base: key.api_base,
            is_active: key.is_active,
            configured_at: key.configured_at.map(|ts| {
                chrono::DateTime::from_timestamp(ts.seconds, ts.nanos as u32)
                    .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
                    .unwrap_or_else(|| "Unknown".to_string())
            }),
        }).collect())
    }
    
    /// Configure an AI provider
    pub async fn configure_provider(
        client: &mut GrpcClient, 
        provider_name: String,
        api_key: String,
        model: Option<String>,
        api_base: Option<String>
    ) -> Result<String> {
        let mut config = prost_types::Struct::default();
        
        // Add provider_type (capitalize first letter of provider_name)
        let provider_type = provider_name.chars()
            .take(1)
            .flat_map(char::to_uppercase)
            .chain(provider_name.chars().skip(1))
            .collect::<String>();
        config.fields.insert("provider_type".to_string(), prost_types::Value {
            kind: Some(prost_types::value::Kind::StringValue(provider_type)),
        });
        
        // Add API key
        config.fields.insert("api_key".to_string(), prost_types::Value {
            kind: Some(prost_types::value::Kind::StringValue(api_key)),
        });
        
        // Add default_model - use provided model or sensible default
        let default_model = if let Some(model) = model {
            model
        } else {
            match provider_name.to_lowercase().as_str() {
                "openai" => "gpt-4.1".to_string(),
                "anthropic" => "claude-4-sonnet-20250514".to_string(),
                _ => "default-model".to_string()
            }
        };
        config.fields.insert("default_model".to_string(), prost_types::Value {
            kind: Some(prost_types::value::Kind::StringValue(default_model)),
        });
        
        // Add API base if provided
        if let Some(api_base) = api_base {
            config.fields.insert("api_base".to_string(), prost_types::Value {
                kind: Some(prost_types::value::Kind::StringValue(api_base)),
            });
        }
        
        // Add empty options field
        let options_struct = prost_types::Struct::default();
        config.fields.insert("options".to_string(), prost_types::Value {
            kind: Some(prost_types::value::Kind::StructValue(options_struct)),
        });
        
        let response = client.ai
            .configure_provider(ConfigureProviderRequest {
                provider_name: provider_name.clone(),
                config: Some(config),
                validate_only: false,
            })
            .await?
            .into_inner();
        
        if response.success {
            Ok(format!("Provider '{}' configured successfully", provider_name))
        } else {
            Err(anyhow::anyhow!(response.error_message))
        }
    }
    
    /// Test a chat message
    pub async fn chat(client: &mut GrpcClient, message: String) -> Result<String> {
        let mut messages = Vec::new();
        messages.push(ChatMessage {
            role: ChatRole::User as i32,
            content: message,
            name: String::new(),
            function_calls: vec![],
            metadata: HashMap::new(),
        });
        
        let response = client.ai
            .generate_chat(GenerateChatRequest {
                messages,
                options: Some(GenerationOptions {
                    model: String::new(),
                    temperature: 0.7,
                    top_p: 1.0,
                    max_tokens: 0,
                    stop_sequences: vec![],
                    presence_penalty: 0.0,
                    frequency_penalty: 0.0,
                    n: 1,
                    stream: false,
                    provider_specific: HashMap::new(),
                }),
                context_id: String::new(),
                function_names: vec![],
            })
            .await?
            .into_inner();
        
        if let Some(completion) = response.completions.first() {
            if let Some(message) = &completion.message {
                Ok(message.content.clone())
            } else {
                Err(anyhow::anyhow!("No message in response"))
            }
        } else {
            Err(anyhow::anyhow!("No completions in response"))
        }
    }
    
    /// List configured providers
    pub async fn list_providers(client: &mut GrpcClient, configured_only: bool) -> Result<Vec<ProviderDisplay>> {
        let response = client.ai
            .list_providers(ListProvidersRequest {
                configured_only,
            })
            .await?
            .into_inner();
        
        Ok(response.providers.into_iter().map(|provider| ProviderDisplay {
            name: provider.name,
            provider_type: match provider.r#type {
                1 => "OpenAI".to_string(),
                2 => "Anthropic".to_string(),
                3 => "Google".to_string(),
                4 => "Local".to_string(),
                5 => "Custom".to_string(),
                _ => "Unknown".to_string(),
            },
            is_configured: provider.is_configured,
            is_active: provider.is_active,
            supported_models: provider.supported_models,
            capabilities: provider.capabilities,
        }).collect())
    }
}

// Display structures for UI formatting
pub struct AiStatusInfo {
    pub enabled: bool,
    pub active_provider: String,
    pub active_models_count: i32,
    pub active_agents_count: i32,
    pub statistics: HashMap<String, String>,
}

pub struct ProviderKeyDisplay {
    pub provider_name: String,
    pub masked_key: String,
    pub api_base: String,
    pub is_active: bool,
    pub configured_at: Option<String>,
}

pub struct ProviderDisplay {
    pub name: String,
    pub provider_type: String,
    pub is_configured: bool,
    pub is_active: bool,
    pub supported_models: Vec<String>,
    pub capabilities: Vec<String>,
}