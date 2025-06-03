use crate::GrpcClient;
use super::CommandError;
use maowbot_proto::maowbot::services::{
    GenerateChatRequest, ChatMessage, ChatRole, GenerationOptions,
    ConfigureProviderRequest, ListProvidersRequest, TestProviderRequest,
    RegisterFunctionRequest, FunctionDefinition, UnregisterFunctionRequest, ListFunctionsRequest,
    SetSystemPromptRequest, GetSystemPromptRequest, ListSystemPromptsRequest,
};
use maowbot_proto::prost_types::Struct as ProtoStruct;
use std::collections::{HashMap, BTreeMap};

/// Result of chat generation
pub struct ChatResult {
    pub response: String,
    pub model_used: String,
    pub tokens_used: i32,
}

/// Result of provider configuration
pub struct ConfigureProviderResult {
    pub success: bool,
    pub message: String,
}

/// Result of listing providers
pub struct ListProvidersResult {
    pub providers: Vec<ProviderInfo>,
    pub active_provider: String,
}

pub struct ProviderInfo {
    pub name: String,
    pub provider_type: String,
    pub is_configured: bool,
    pub is_active: bool,
    pub supported_models: Vec<String>,
}

/// Result of testing a provider
pub struct TestProviderResult {
    pub success: bool,
    pub response: String,
    pub latency_ms: i64,
}

/// Result of listing functions
pub struct ListFunctionsResult {
    pub functions: Vec<FunctionInfo>,
}

pub struct FunctionInfo {
    pub name: String,
    pub description: String,
    pub is_enabled: bool,
    pub call_count: i64,
}

/// Result of listing system prompts
pub struct ListSystemPromptsResult {
    pub prompts: Vec<SystemPromptInfo>,
    pub active_prompt_id: String,
}

pub struct SystemPromptInfo {
    pub prompt_id: String,
    pub prompt: String,
}

/// AI command handlers
pub struct AICommands;

impl AICommands {
    /// Generate a chat response
    pub async fn generate_chat(
        client: &GrpcClient,
        message: &str,
        context_id: Option<&str>,
    ) -> Result<ChatResult, CommandError> {
        let chat_message = ChatMessage {
            role: ChatRole::User as i32,
            content: message.to_string(),
            name: String::new(),
            function_calls: vec![],
            metadata: HashMap::new(),
        };
        
        let request = GenerateChatRequest {
            messages: vec![chat_message],
            options: Some(GenerationOptions {
                model: String::new(), // Use default
                temperature: 0.7,
                top_p: 1.0,
                max_tokens: 0, // Use default
                stop_sequences: vec![],
                presence_penalty: 0.0,
                frequency_penalty: 0.0,
                n: 1,
                stream: false,
                provider_specific: HashMap::new(),
            }),
            context_id: context_id.unwrap_or_default().to_string(),
            function_names: vec![],
        };
        
        let mut client = client.ai.clone();
        let response = client
            .generate_chat(request)
            .await
            .map_err(|e| CommandError::GrpcError(e.to_string()))?;
            
        let response = response.into_inner();
        
        if let Some(completion) = response.completions.first() {
            if let Some(message) = &completion.message {
                Ok(ChatResult {
                    response: message.content.clone(),
                    model_used: response.model_used,
                    tokens_used: response.usage.as_ref().map(|u| u.total_tokens).unwrap_or(0),
                })
            } else {
                Err(CommandError::DataError("No message in completion".to_string()))
            }
        } else {
            Err(CommandError::DataError("No completions returned".to_string()))
        }
    }
    
    /// Configure an AI provider
    pub async fn configure_provider(
        client: &GrpcClient,
        provider_name: &str,
        config: HashMap<String, String>,
    ) -> Result<ConfigureProviderResult, CommandError> {
        // Convert HashMap to protobuf Struct
        let mut fields = BTreeMap::new();
        for (key, value) in config {
            fields.insert(key, maowbot_proto::prost_types::Value {
                kind: Some(maowbot_proto::prost_types::value::Kind::StringValue(value)),
            });
        }
        
        let proto_config = ProtoStruct { fields };
        
        let request = ConfigureProviderRequest {
            provider_name: provider_name.to_string(),
            config: Some(proto_config),
            validate_only: false,
        };
        
        let mut client = client.ai.clone();
        let response = client
            .configure_provider(request)
            .await
            .map_err(|e| CommandError::GrpcError(e.to_string()))?;
            
        let response = response.into_inner();
        
        Ok(ConfigureProviderResult {
            success: response.success,
            message: if response.success {
                format!("Provider '{}' configured successfully", provider_name)
            } else {
                response.error_message
            },
        })
    }
    
    /// List configured providers
    pub async fn list_providers(
        client: &GrpcClient,
        configured_only: bool,
    ) -> Result<ListProvidersResult, CommandError> {
        let request = ListProvidersRequest { configured_only };
        
        let mut client = client.ai.clone();
        let response = client
            .list_providers(request)
            .await
            .map_err(|e| CommandError::GrpcError(e.to_string()))?;
            
        let response = response.into_inner();
        
        let providers = response.providers.into_iter().map(|p| ProviderInfo {
            name: p.name,
            provider_type: format!("{:?}", p.r#type),
            is_configured: p.is_configured,
            is_active: p.is_active,
            supported_models: p.supported_models,
        }).collect();
        
        Ok(ListProvidersResult {
            providers,
            active_provider: response.active_provider,
        })
    }
    
    /// Test a provider configuration
    pub async fn test_provider(
        client: &GrpcClient,
        provider_name: &str,
        test_prompt: Option<&str>,
    ) -> Result<TestProviderResult, CommandError> {
        let request = TestProviderRequest {
            provider_name: provider_name.to_string(),
            test_prompt: test_prompt.unwrap_or("Hello, this is a test.").to_string(),
        };
        
        let mut client = client.ai.clone();
        let response = client
            .test_provider(request)
            .await
            .map_err(|e| CommandError::GrpcError(e.to_string()))?;
            
        let response = response.into_inner();
        
        Ok(TestProviderResult {
            success: response.success,
            response: if response.success {
                response.response
            } else {
                response.error_message
            },
            latency_ms: response.latency_ms,
        })
    }
    
    /// Register a function for AI use
    pub async fn register_function(
        client: &GrpcClient,
        name: &str,
        description: &str,
    ) -> Result<(), CommandError> {
        let function = FunctionDefinition {
            name: name.to_string(),
            description: description.to_string(),
            parameters: None, // TODO: Add parameter support
            required_parameters: vec![],
            examples: HashMap::new(),
        };
        
        let request = RegisterFunctionRequest {
            function: Some(function),
        };
        
        let mut client = client.ai.clone();
        client
            .register_function(request)
            .await
            .map_err(|e| CommandError::GrpcError(e.to_string()))?;
            
        Ok(())
    }
    
    /// Unregister a function
    pub async fn unregister_function(
        client: &GrpcClient,
        function_name: &str,
    ) -> Result<(), CommandError> {
        let request = UnregisterFunctionRequest {
            function_name: function_name.to_string(),
        };
        
        let mut client = client.ai.clone();
        client
            .unregister_function(request)
            .await
            .map_err(|e| CommandError::GrpcError(e.to_string()))?;
            
        Ok(())
    }
    
    /// List registered functions
    pub async fn list_functions(
        client: &GrpcClient,
        categories: Vec<String>,
    ) -> Result<ListFunctionsResult, CommandError> {
        let request = ListFunctionsRequest { categories };
        
        let mut client = client.ai.clone();
        let response = client
            .list_functions(request)
            .await
            .map_err(|e| CommandError::GrpcError(e.to_string()))?;
            
        let response = response.into_inner();
        
        let functions = response.functions.into_iter().map(|f| {
            let def = f.definition.unwrap_or_default();
            FunctionInfo {
                name: def.name,
                description: def.description,
                is_enabled: f.is_enabled,
                call_count: f.call_count,
            }
        }).collect();
        
        Ok(ListFunctionsResult { functions })
    }
    
    /// Set system prompt
    pub async fn set_system_prompt(
        client: &GrpcClient,
        prompt_id: &str,
        prompt: &str,
    ) -> Result<(), CommandError> {
        let request = SetSystemPromptRequest {
            prompt_id: prompt_id.to_string(),
            prompt: prompt.to_string(),
            variables: HashMap::new(),
        };
        
        let mut client = client.ai.clone();
        client
            .set_system_prompt(request)
            .await
            .map_err(|e| CommandError::GrpcError(e.to_string()))?;
            
        Ok(())
    }
    
    /// Get system prompt
    pub async fn get_system_prompt(
        client: &GrpcClient,
        prompt_id: &str,
    ) -> Result<String, CommandError> {
        let request = GetSystemPromptRequest {
            prompt_id: prompt_id.to_string(),
        };
        
        let mut client = client.ai.clone();
        let response = client
            .get_system_prompt(request)
            .await
            .map_err(|e| CommandError::GrpcError(e.to_string()))?;
            
        let response = response.into_inner();
        
        if let Some(prompt) = response.prompt {
            Ok(prompt.prompt)
        } else {
            Err(CommandError::NotFound(format!("Prompt '{}' not found", prompt_id)))
        }
    }
    
    /// List system prompts
    pub async fn list_system_prompts(
        client: &GrpcClient,
    ) -> Result<ListSystemPromptsResult, CommandError> {
        let request = ListSystemPromptsRequest {};
        
        let mut client = client.ai.clone();
        let response = client
            .list_system_prompts(request)
            .await
            .map_err(|e| CommandError::GrpcError(e.to_string()))?;
            
        let response = response.into_inner();
        
        let prompts = response.prompts.into_iter().map(|p| SystemPromptInfo {
            prompt_id: p.prompt_id,
            prompt: p.prompt,
        }).collect();
        
        Ok(ListSystemPromptsResult {
            prompts,
            active_prompt_id: response.active_prompt_id,
        })
    }
}