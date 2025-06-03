use tonic::{Request, Response, Status};
use maowbot_proto::maowbot::services::{ai_service_server::AiService, *};
use maowbot_common::traits::api::AiApi;
use maowbot_core::plugins::manager::ai_api_impl::AiApiImpl;
use std::sync::Arc;
use std::collections::HashMap;
use uuid::Uuid;
use chrono::Utc;
use tracing::{info, error, debug};
use prost_types;
use serde_json;

// Helper function to convert protobuf Struct to serde_json::Value
fn struct_to_json_value(s: &prost_types::Struct) -> serde_json::Value {
    let mut map = serde_json::Map::new();
    for (k, v) in &s.fields {
        map.insert(k.clone(), value_to_json(&v));
    }
    serde_json::Value::Object(map)
}

fn value_to_json(v: &prost_types::Value) -> serde_json::Value {
    match &v.kind {
        Some(prost_types::value::Kind::NullValue(_)) => serde_json::Value::Null,
        Some(prost_types::value::Kind::NumberValue(n)) => serde_json::json!(n),
        Some(prost_types::value::Kind::StringValue(s)) => serde_json::Value::String(s.clone()),
        Some(prost_types::value::Kind::BoolValue(b)) => serde_json::Value::Bool(*b),
        Some(prost_types::value::Kind::StructValue(s)) => struct_to_json_value(s),
        Some(prost_types::value::Kind::ListValue(l)) => {
            serde_json::Value::Array(l.values.iter().map(value_to_json).collect())
        }
        None => serde_json::Value::Null,
    }
}

pub struct AiServiceImpl {
    ai_api: Option<Arc<AiApiImpl>>,
}

impl AiServiceImpl {
    pub fn new() -> Self {
        Self {
            ai_api: None,
        }
    }
    
    pub fn new_with_api(ai_api: Arc<AiApiImpl>) -> Self {
        Self {
            ai_api: Some(ai_api),
        }
    }
    
    fn messages_to_json(messages: &[ChatMessage]) -> Vec<serde_json::Value> {
        messages.iter().map(|msg| {
            let mut obj = serde_json::Map::new();
            obj.insert("role".to_string(), serde_json::Value::String(
                match msg.role() {
                    ChatRole::System => "system",
                    ChatRole::User => "user",
                    ChatRole::Assistant => "assistant",
                    ChatRole::Function => "function",
                    _ => "user",
                }.to_string()
            ));
            obj.insert("content".to_string(), serde_json::Value::String(msg.content.clone()));
            
            if !msg.name.is_empty() {
                obj.insert("name".to_string(), serde_json::Value::String(msg.name.clone()));
            }
            
            if !msg.function_calls.is_empty() {
                let calls: Vec<serde_json::Value> = msg.function_calls.iter().map(|fc| {
                    let args_json = if let Some(ref args) = fc.arguments {
                        struct_to_json_value(args)
                    } else {
                        serde_json::Value::Null
                    };
                    serde_json::json!({
                        "name": fc.name,
                        "arguments": args_json,
                        "id": fc.id,
                    })
                }).collect();
                obj.insert("function_calls".to_string(), serde_json::Value::Array(calls));
            }
            
            serde_json::Value::Object(obj)
        }).collect()
    }
}

#[tonic::async_trait]
impl AiService for AiServiceImpl {
    async fn generate_chat(&self, request: Request<GenerateChatRequest>) -> Result<Response<GenerateChatResponse>, Status> {
        let req = request.into_inner();
        debug!("Generating chat with {} messages", req.messages.len());
        
        let ai_api = self.ai_api.as_ref()
            .ok_or_else(|| Status::failed_precondition("AI service not configured"))?;
        
        // Convert proto messages to JSON
        let json_messages = Self::messages_to_json(&req.messages);
        
        // Generate the chat response
        let response = ai_api.generate_chat(json_messages).await
            .map_err(|e| Status::internal(format!("Failed to generate chat: {}", e)))?;
        
        // Build the response
        let completion = ChatCompletion {
            message: Some(ChatMessage {
                role: ChatRole::Assistant as i32,
                content: response,
                name: String::new(),
                function_calls: vec![],
                metadata: HashMap::new(),
            }),
            finish_reason: FinishReason::Stop as i32,
            index: 0,
        };
        
        // TODO: Get real usage info
        let usage = UsageInfo {
            prompt_tokens: 0,
            completion_tokens: 0,
            total_tokens: 0,
            estimated_cost: 0.0,
        };
        
        Ok(Response::new(GenerateChatResponse {
            completions: vec![completion],
            usage: Some(usage),
            model_used: req.options.as_ref().map(|o| o.model.clone()).unwrap_or_default(),
            request_id: Uuid::new_v4().to_string(),
        }))
    }
    type StreamGenerateChatStream = tonic::codec::Streaming<ChatToken>;
    async fn stream_generate_chat(&self, _: Request<StreamGenerateChatRequest>) -> Result<Response<Self::StreamGenerateChatStream>, Status> {
        // TODO: Implement streaming chat generation
        Err(Status::unimplemented("Streaming chat generation not yet implemented"))
    }
    async fn configure_provider(&self, request: Request<ConfigureProviderRequest>) -> Result<Response<ConfigureProviderResponse>, Status> {
        let req = request.into_inner();
        info!("Configuring AI provider: {}", req.provider_name);
        
        let ai_api = self.ai_api.as_ref()
            .ok_or_else(|| Status::failed_precondition("AI service not configured"))?;
        
        if req.validate_only {
            // Just validate without applying
            return Ok(Response::new(ConfigureProviderResponse {
                success: true,
                error_message: String::new(),
                provider: Some(ProviderInfo {
                    name: req.provider_name,
                    r#type: ProviderType::Custom as i32,
                    is_configured: true,
                    is_active: false,
                    supported_models: vec![],
                    capabilities: vec!["chat".to_string()],
                    configured_at: Some(prost_types::Timestamp {
                        seconds: Utc::now().timestamp(),
                        nanos: 0,
                    }),
                }),
            }));
        }
        
        // Convert protobuf struct to JSON
        let config_json = if let Some(config) = req.config {
            struct_to_json_value(&config)
        } else {
            serde_json::Value::Object(serde_json::Map::new())
        };
        
        // Configure the provider
        match ai_api.configure_ai_provider(config_json).await {
            Ok(_) => {
                Ok(Response::new(ConfigureProviderResponse {
                    success: true,
                    error_message: String::new(),
                    provider: Some(ProviderInfo {
                        name: req.provider_name,
                        r#type: ProviderType::Custom as i32,
                        is_configured: true,
                        is_active: true,
                        supported_models: vec![],
                        capabilities: vec!["chat".to_string()],
                        configured_at: Some(prost_types::Timestamp {
                            seconds: Utc::now().timestamp(),
                            nanos: 0,
                        }),
                    }),
                }))
            }
            Err(e) => {
                Ok(Response::new(ConfigureProviderResponse {
                    success: false,
                    error_message: format!("{}", e),
                    provider: None,
                }))
            }
        }
    }
    async fn get_provider_config(&self, request: Request<GetProviderConfigRequest>) -> Result<Response<GetProviderConfigResponse>, Status> {
        let req = request.into_inner();
        debug!("Getting provider config for: {}", req.provider_name);
        
        // TODO: Store and retrieve actual provider configs
        // For now, return a mock response
        let provider = ProviderInfo {
            name: req.provider_name,
            r#type: ProviderType::Custom as i32,
            is_configured: true,
            is_active: true,
            supported_models: vec!["gpt-3.5-turbo".to_string(), "gpt-4".to_string()],
            capabilities: vec!["chat".to_string(), "functions".to_string()],
            configured_at: Some(prost_types::Timestamp {
                seconds: Utc::now().timestamp() - 3600,
                nanos: 0,
            }),
        };
        
        let mut config = prost_types::Struct::default();
        if !req.include_secrets {
            // Redact sensitive information
            config.fields.insert("api_key".to_string(), prost_types::Value {
                kind: Some(prost_types::value::Kind::StringValue("<redacted>".to_string())),
            });
        }
        
        Ok(Response::new(GetProviderConfigResponse {
            provider: Some(provider),
            config: Some(config),
        }))
    }
    async fn list_providers(&self, request: Request<ListProvidersRequest>) -> Result<Response<ListProvidersResponse>, Status> {
        let req = request.into_inner();
        debug!("Listing providers - configured_only: {}", req.configured_only);
        
        // TODO: Implement actual provider listing
        // For now, return mock providers
        let mut providers = vec![
            ProviderInfo {
                name: "openai".to_string(),
                r#type: ProviderType::Openai as i32,
                is_configured: true,
                is_active: true,
                supported_models: vec!["gpt-3.5-turbo".to_string(), "gpt-4".to_string()],
                capabilities: vec!["chat".to_string(), "functions".to_string(), "embeddings".to_string()],
                configured_at: Some(prost_types::Timestamp {
                    seconds: Utc::now().timestamp() - 86400,
                    nanos: 0,
                }),
            },
            ProviderInfo {
                name: "anthropic".to_string(),
                r#type: ProviderType::Anthropic as i32,
                is_configured: false,
                is_active: false,
                supported_models: vec!["claude-3-sonnet".to_string(), "claude-3-opus".to_string()],
                capabilities: vec!["chat".to_string()],
                configured_at: None,
            },
        ];
        
        if req.configured_only {
            providers.retain(|p| p.is_configured);
        }
        
        Ok(Response::new(ListProvidersResponse {
            providers,
            active_provider: "openai".to_string(),
        }))
    }
    async fn test_provider(&self, request: Request<TestProviderRequest>) -> Result<Response<TestProviderResponse>, Status> {
        let req = request.into_inner();
        info!("Testing provider: {}", req.provider_name);
        
        let ai_api = self.ai_api.as_ref()
            .ok_or_else(|| Status::failed_precondition("AI service not configured"))?;
        
        let test_prompt = if req.test_prompt.is_empty() {
            "Hello, please respond with 'Test successful' if you can read this."
        } else {
            &req.test_prompt
        };
        
        let start = std::time::Instant::now();
        
        // Try to generate a response
        let messages = vec![serde_json::json!({
            "role": "user",
            "content": test_prompt
        })];
        
        match ai_api.generate_chat(messages).await {
            Ok(response) => {
                let latency_ms = start.elapsed().as_millis() as i64;
                Ok(Response::new(TestProviderResponse {
                    success: true,
                    response,
                    error_message: String::new(),
                    latency_ms,
                }))
            }
            Err(e) => {
                Ok(Response::new(TestProviderResponse {
                    success: false,
                    response: String::new(),
                    error_message: format!("Provider test failed: {}", e),
                    latency_ms: 0,
                }))
            }
        }
    }
    async fn register_function(&self, request: Request<RegisterFunctionRequest>) -> Result<Response<()>, Status> {
        let req = request.into_inner();
        let function = req.function
            .ok_or_else(|| Status::invalid_argument("Function definition required"))?;
        
        info!("Registering AI function: {}", function.name);
        
        let ai_api = self.ai_api.as_ref()
            .ok_or_else(|| Status::failed_precondition("AI service not configured"))?;
        
        ai_api.register_ai_function(&function.name, &function.description).await
            .map_err(|e| Status::internal(format!("Failed to register function: {}", e)))?;
        
        Ok(Response::new(()))
    }
    async fn unregister_function(&self, request: Request<UnregisterFunctionRequest>) -> Result<Response<()>, Status> {
        let req = request.into_inner();
        info!("Unregistering AI function: {}", req.function_name);
        
        // TODO: Implement function unregistration
        debug!("Function unregistration not yet implemented");
        
        Ok(Response::new(()))
    }
    async fn list_functions(&self, request: Request<ListFunctionsRequest>) -> Result<Response<ListFunctionsResponse>, Status> {
        let req = request.into_inner();
        debug!("Listing functions - categories: {:?}", req.categories);
        
        // TODO: Implement actual function listing
        // For now, return mock functions
        let functions = vec![
            FunctionInfo {
                definition: Some(FunctionDefinition {
                    name: "get_weather".to_string(),
                    description: "Get current weather for a location".to_string(),
                    parameters: Some(prost_types::Struct::default()),
                    required_parameters: vec![],
                    examples: HashMap::new(),
                }),
                is_enabled: true,
                call_count: 10,
                last_called: Some(prost_types::Timestamp {
                    seconds: Utc::now().timestamp() - 3600,
                    nanos: 0,
                }),
            },
            FunctionInfo {
                definition: Some(FunctionDefinition {
                    name: "search_web".to_string(),
                    description: "Search the web for information".to_string(),
                    parameters: Some(prost_types::Struct::default()),
                    required_parameters: vec![],
                    examples: HashMap::new(),
                }),
                is_enabled: true,
                call_count: 5,
                last_called: Some(prost_types::Timestamp {
                    seconds: Utc::now().timestamp() - 7200,
                    nanos: 0,
                }),
            },
        ];
        
        // TODO: Filter by categories if needed
        
        Ok(Response::new(ListFunctionsResponse {
            functions,
        }))
    }
    async fn call_function(&self, request: Request<CallFunctionRequest>) -> Result<Response<CallFunctionResponse>, Status> {
        let req = request.into_inner();
        debug!("Calling function: {}", req.function_name);
        
        // TODO: Implement actual function calling
        // For now, return a mock response
        let result = prost_types::Value {
            kind: Some(prost_types::value::Kind::StringValue(
                format!("Mock result for function '{}'", req.function_name)
            )),
        };
        
        Ok(Response::new(CallFunctionResponse {
            success: true,
            result: Some(result),
            error_message: String::new(),
            execution_time_ms: 42,
        }))
    }
    async fn set_system_prompt(&self, request: Request<SetSystemPromptRequest>) -> Result<Response<()>, Status> {
        let req = request.into_inner();
        info!("Setting system prompt: {}", req.prompt_id);
        
        let ai_api = self.ai_api.as_ref()
            .ok_or_else(|| Status::failed_precondition("AI service not configured"))?;
        
        ai_api.set_system_prompt(&req.prompt).await
            .map_err(|e| Status::internal(format!("Failed to set system prompt: {}", e)))?;
        
        Ok(Response::new(()))
    }
    async fn get_system_prompt(&self, request: Request<GetSystemPromptRequest>) -> Result<Response<GetSystemPromptResponse>, Status> {
        let req = request.into_inner();
        debug!("Getting system prompt: {}", req.prompt_id);
        
        // TODO: Implement actual system prompt retrieval
        // For now, return a mock prompt
        let prompt = SystemPrompt {
            prompt_id: req.prompt_id,
            prompt: "You are a helpful assistant.".to_string(),
            variables: HashMap::new(),
            created_at: Some(prost_types::Timestamp {
                seconds: Utc::now().timestamp() - 86400,
                nanos: 0,
            }),
            updated_at: Some(prost_types::Timestamp {
                seconds: Utc::now().timestamp() - 3600,
                nanos: 0,
            }),
        };
        
        Ok(Response::new(GetSystemPromptResponse {
            prompt: Some(prompt),
        }))
    }
    async fn list_system_prompts(&self, _: Request<ListSystemPromptsRequest>) -> Result<Response<ListSystemPromptsResponse>, Status> {
        debug!("Listing system prompts");
        
        // TODO: Implement actual system prompt listing
        // For now, return mock prompts
        let prompts = vec![
            SystemPrompt {
                prompt_id: "default".to_string(),
                prompt: "You are a helpful assistant.".to_string(),
                variables: HashMap::new(),
                created_at: Some(prost_types::Timestamp {
                    seconds: Utc::now().timestamp() - 86400,
                    nanos: 0,
                }),
                updated_at: Some(prost_types::Timestamp {
                    seconds: Utc::now().timestamp() - 3600,
                    nanos: 0,
                }),
            },
            SystemPrompt {
                prompt_id: "coding".to_string(),
                prompt: "You are an expert programming assistant.".to_string(),
                variables: HashMap::new(),
                created_at: Some(prost_types::Timestamp {
                    seconds: Utc::now().timestamp() - 172800,
                    nanos: 0,
                }),
                updated_at: Some(prost_types::Timestamp {
                    seconds: Utc::now().timestamp() - 86400,
                    nanos: 0,
                }),
            },
        ];
        
        Ok(Response::new(ListSystemPromptsResponse {
            prompts,
            active_prompt_id: "default".to_string(),
        }))
    }
    async fn create_memory(&self, request: Request<CreateMemoryRequest>) -> Result<Response<CreateMemoryResponse>, Status> {
        let req = request.into_inner();
        let input_memory = req.memory.ok_or_else(|| Status::invalid_argument("Memory data is required"))?;
        info!("Creating memory for user: {:?}", input_memory.user_id);
        
        // TODO: Implement actual memory storage
        // For now, return a mock response
        let memory_id = Uuid::new_v4().to_string();
        let memory = Memory {
            memory_id: memory_id.clone(),
            user_id: input_memory.user_id,
            content: input_memory.content,
            r#type: input_memory.r#type,
            tags: input_memory.tags,
            metadata: input_memory.metadata,
            created_at: Some(prost_types::Timestamp {
                seconds: Utc::now().timestamp(),
                nanos: 0,
            }),
            accessed_at: Some(prost_types::Timestamp {
                seconds: Utc::now().timestamp(),
                nanos: 0,
            }),
            access_count: 0,
            importance_score: input_memory.importance_score,
        };
        
        Ok(Response::new(CreateMemoryResponse {
            memory: Some(memory),
        }))
    }
    async fn get_memory(&self, request: Request<GetMemoryRequest>) -> Result<Response<GetMemoryResponse>, Status> {
        let req = request.into_inner();
        debug!("Getting memory: {}", req.memory_id);
        
        // TODO: Implement actual memory retrieval
        Err(Status::not_found("Memory not found"))
    }
    async fn update_memory(&self, request: Request<UpdateMemoryRequest>) -> Result<Response<UpdateMemoryResponse>, Status> {
        let req = request.into_inner();
        debug!("Updating memory: {}", req.memory_id);
        
        // TODO: Implement actual memory update
        Err(Status::not_found("Memory not found"))
    }
    async fn delete_memory(&self, request: Request<DeleteMemoryRequest>) -> Result<Response<()>, Status> {
        let req = request.into_inner();
        info!("Deleting memory: {}", req.memory_id);
        
        // TODO: Implement actual memory deletion
        Ok(Response::new(()))
    }
    async fn search_memories(&self, request: Request<SearchMemoriesRequest>) -> Result<Response<SearchMemoriesResponse>, Status> {
        let req = request.into_inner();
        debug!("Searching memories - query: {}, user: {:?}", req.query, req.user_id);
        
        // TODO: Implement actual memory search
        Ok(Response::new(SearchMemoriesResponse {
            results: vec![],
        }))
    }
    async fn create_context(&self, request: Request<CreateContextRequest>) -> Result<Response<CreateContextResponse>, Status> {
        let req = request.into_inner();
        let input_context = req.context.ok_or_else(|| Status::invalid_argument("Context data is required"))?;
        info!("Creating context for user: {:?}", input_context.user_id);
        
        // TODO: Implement actual context creation
        let context_id = Uuid::new_v4().to_string();
        let context = Context {
            context_id: context_id.clone(),
            user_id: input_context.user_id,
            messages: input_context.messages,
            memory_ids: input_context.memory_ids,
            variables: input_context.variables,
            created_at: Some(prost_types::Timestamp {
                seconds: Utc::now().timestamp(),
                nanos: 0,
            }),
            updated_at: Some(prost_types::Timestamp {
                seconds: Utc::now().timestamp(),
                nanos: 0,
            }),
            message_count: 0,
            token_count: 0,
        };
        
        Ok(Response::new(CreateContextResponse {
            context: Some(context),
        }))
    }
    async fn get_context(&self, request: Request<GetContextRequest>) -> Result<Response<GetContextResponse>, Status> {
        let req = request.into_inner();
        debug!("Getting context: {}", req.context_id);
        
        // TODO: Implement actual context retrieval
        Err(Status::not_found("Context not found"))
    }
    async fn update_context(&self, request: Request<UpdateContextRequest>) -> Result<Response<UpdateContextResponse>, Status> {
        let req = request.into_inner();
        debug!("Updating context: {}", req.context_id);
        
        // TODO: Implement actual context update
        Err(Status::not_found("Context not found"))
    }
    async fn clear_context(&self, request: Request<ClearContextRequest>) -> Result<Response<()>, Status> {
        let req = request.into_inner();
        info!("Clearing context: {}", req.context_id);
        
        // TODO: Implement actual context clearing
        Ok(Response::new(()))
    }
    async fn get_ai_usage(&self, request: Request<GetAiUsageRequest>) -> Result<Response<GetAiUsageResponse>, Status> {
        let req = request.into_inner();
        debug!("Getting AI usage statistics");
        
        // TODO: Implement actual usage tracking
        // For now, return mock data
        let usage_data = vec![
            UsageEntry {
                user_id: req.user_id.clone(),
                provider: "openai".to_string(),
                model: "gpt-3.5-turbo".to_string(),
                request_count: 42,
                total_tokens: 12345,
                total_cost: 0.025,
                timestamp: Some(prost_types::Timestamp {
                    seconds: Utc::now().timestamp(),
                    nanos: 0,
                }),
            },
        ];
        
        let mut tokens_by_model = HashMap::new();
        tokens_by_model.insert("gpt-3.5-turbo".to_string(), 12345);
        
        let mut cost_by_model = HashMap::new();
        cost_by_model.insert("gpt-3.5-turbo".to_string(), 0.025);
        
        let summary = UsageSummary {
            total_requests: 42,
            total_tokens: 12345,
            total_cost: 0.025,
            tokens_by_model,
            cost_by_model,
        };
        
        Ok(Response::new(GetAiUsageResponse {
            usage: usage_data,
            summary: Some(summary),
        }))
    }
    async fn get_model_performance(&self, request: Request<GetModelPerformanceRequest>) -> Result<Response<GetModelPerformanceResponse>, Status> {
        let req = request.into_inner();
        debug!("Getting model performance for: {:?}", req.model);
        
        // TODO: Implement actual performance tracking
        // For now, return mock data
        let performance_data = vec![
            ModelPerformance {
                model: "gpt-3.5-turbo".to_string(),
                average_latency_ms: 350.0,
                p95_latency_ms: 500.0,
                p99_latency_ms: 800.0,
                success_rate: 0.95,
                average_tokens_per_second: 150.0,
                sample_count: 100,
            },
            ModelPerformance {
                model: "gpt-4".to_string(),
                average_latency_ms: 1200.0,
                p95_latency_ms: 2000.0,
                p99_latency_ms: 3000.0,
                success_rate: 0.98,
                average_tokens_per_second: 50.0,
                sample_count: 100,
            },
        ];
        
        Ok(Response::new(GetModelPerformanceResponse {
            models: performance_data,
        }))
    }
}