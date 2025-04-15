use std::sync::Arc;

use anyhow::anyhow;
use async_trait::async_trait;
use tokio::sync::RwLock;
use uuid::Uuid;
use chrono::Utc;
use tracing::{debug, error, info, trace};
use maowbot_common::error::Error as MaowError;
use maowbot_common::models::analytics::BotEvent;
use maowbot_common::models::platform::Platform;
use maowbot_common::models::user::User;
use maowbot_common::models::ai::{
    AiProvider, AiCredential, AiModel, AiTrigger, AiMemory, 
    AiTriggerWithDetails, AiAgent, AiAction, AiSystemPrompt, 
    AiAgentWithDetails, TriggerType, MemoryRole, ActionHandlerType
};
use crate::traits::{AiApi, ChatResponse};
use maowbot_common::traits::repository_traits::{
    CredentialsRepository, UserRepo, AiProviderRepository, AiCredentialRepository,
    AiModelRepository, AiTriggerRepository, AiMemoryRepository, AiConfigurationRepository,
    AiAgentRepository, AiActionRepository, AiSystemPromptRepository
};

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
    /// Whether AI is enabled
    enabled: RwLock<bool>,
    /// User repository for looking up users
    user_repo: Arc<dyn UserRepo + Send + Sync>,
    /// Credentials repository
    cred_repo: Arc<dyn CredentialsRepository + Send + Sync>,
    
    // AI Repositories
    /// AI provider repository
    provider_repo: Option<Arc<dyn AiProviderRepository + Send + Sync>>,
    /// AI credential repository
    ai_credential_repo: Option<Arc<dyn AiCredentialRepository + Send + Sync>>,
    /// AI model repository
    model_repo: Option<Arc<dyn AiModelRepository + Send + Sync>>,
    /// AI trigger repository
    trigger_repo: Option<Arc<dyn AiTriggerRepository + Send + Sync>>,
    /// AI memory repository
    memory_repo: Option<Arc<dyn AiMemoryRepository + Send + Sync>>,
    /// AI agent repository
    agent_repo: Option<Arc<dyn AiAgentRepository + Send + Sync>>,
    /// AI action repository
    action_repo: Option<Arc<dyn AiActionRepository + Send + Sync>>,
    /// AI system prompt repository
    prompt_repo: Option<Arc<dyn AiSystemPromptRepository + Send + Sync>>,
    /// AI configuration repository
    config_repo: Option<Arc<dyn AiConfigurationRepository + Send + Sync>>,
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
            enabled: RwLock::new(true),
            user_repo,
            cred_repo,
            provider_repo: None,
            ai_credential_repo: None,
            model_repo: None,
            trigger_repo: None,
            memory_repo: None, 
            agent_repo: None,
            action_repo: None,
            prompt_repo: None,
            config_repo: None,
        })
    }
    
    /// Create a new AI service with database repositories
    pub async fn with_repositories(
        user_repo: Arc<dyn UserRepo + Send + Sync>,
        cred_repo: Arc<dyn CredentialsRepository + Send + Sync>,
        provider_repo: Arc<dyn AiProviderRepository + Send + Sync>,
        ai_credential_repo: Arc<dyn AiCredentialRepository + Send + Sync>,
        model_repo: Arc<dyn AiModelRepository + Send + Sync>,
        trigger_repo: Arc<dyn AiTriggerRepository + Send + Sync>,
        memory_repo: Arc<dyn AiMemoryRepository + Send + Sync>,
        agent_repo: Arc<dyn AiAgentRepository + Send + Sync>,
        action_repo: Arc<dyn AiActionRepository + Send + Sync>,
        prompt_repo: Arc<dyn AiSystemPromptRepository + Send + Sync>,
        config_repo: Arc<dyn AiConfigurationRepository + Send + Sync>,
    ) -> anyhow::Result<Self> {
        info!("🔧 AI SERVICE: with_repositories called - setting up AI service with database integration");
        
        // Create a basic service first
        info!("🔧 AI SERVICE: Creating basic service with user_repo and cred_repo");
        let service = match Self::new(user_repo, cred_repo).await {
            Ok(svc) => {
                info!("🔧 AI SERVICE: Basic service created successfully");
                svc
            },
            Err(e) => {
                error!("🔧 AI SERVICE: Failed to create basic service: {:?}", e);
                return Err(e);
            }
        };
        
        info!("🔧 AI SERVICE: Attaching AI repositories");
        
        // Add the repositories
        let mut service = service;
        service.provider_repo = Some(provider_repo);
        service.ai_credential_repo = Some(ai_credential_repo);
        service.model_repo = Some(model_repo);
        service.trigger_repo = Some(trigger_repo);
        service.memory_repo = Some(memory_repo);
        service.agent_repo = Some(agent_repo);
        service.action_repo = Some(action_repo);
        service.prompt_repo = Some(prompt_repo);
        service.config_repo = Some(config_repo);
        
        // Initialize from database
        info!("🔧 AI SERVICE: Initializing from database");
        if let Err(e) = service.initialize_from_database().await {
            error!("🔧 AI SERVICE: Failed to initialize AI service from database: {:?}", e);
            // Continue anyway - we can still function with just the basic service
        } else {
            info!("🔧 AI SERVICE: Database initialization successful");
        }
        
        // Log the enabled status 
        let enabled = *service.enabled.read().await;
        info!("🔧 AI SERVICE: Service initialization complete - service enabled: {}", enabled);
        
        Ok(service)
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
    
    /// Initialize data from database
    pub async fn initialize_from_database(&self) -> anyhow::Result<()> {
        info!("🔍 AI SERVICE: initialize_from_database called");
        
        // Load configuration
        if let Some(config_repo) = &self.config_repo {
            info!("🔍 AI SERVICE: Loading configuration from config_repo");
            // Get default configuration
            match config_repo.get_default_configuration().await {
                Ok(Some(config)) => {
                    info!("🔍 AI SERVICE: Found default config: provider={}, model={}", 
                                 config.provider.name, config.model.name);
                    
                    // Create provider config
                    let provider_config = ProviderConfig {
                        provider_type: config.provider.name.clone(),
                        api_key: config.credential.api_key.clone(),
                        default_model: config.model.name.clone(),
                        api_base: config.credential.api_base.clone(),
                        options: std::collections::HashMap::new(),
                    };
                    
                    // Configure with this provider
                    info!("🔍 AI SERVICE: Configuring provider: {}", provider_config.provider_type);
                    if let Err(e) = self.internal_configure_provider(provider_config).await {
                        error!("🔍 AI SERVICE: Failed to configure provider from database: {:?}", e);
                        // Continue anyway - we'll try other initialization steps
                    } else {
                        info!("🔍 AI SERVICE: Provider configured successfully");
                    }
                },
                Ok(None) => {
                    info!("🔍 AI SERVICE: No default AI configuration found in database");
                },
                Err(e) => {
                    error!("🔍 AI SERVICE: Error loading default configuration: {:?}", e);
                    // Continue anyway - we'll try other initialization steps
                }
            }
        } else {
            info!("🔍 AI SERVICE: No config_repo available, skipping configuration loading");
        }
        
        // Load triggers
        if let Some(trigger_repo) = &self.trigger_repo {
            info!("🔍 AI SERVICE: Loading triggers from trigger_repo");
            // Load all triggers
            match trigger_repo.list_triggers_with_details().await {
                Ok(triggers) => {
                    info!("🔍 AI SERVICE: Loaded {} triggers from database", triggers.len());
                    
                    // Only process prefix triggers for now
                    for trigger_details in triggers {
                        let trigger = trigger_details.trigger;
                        
                        // Handle prefix triggers
                        if trigger.trigger_type == TriggerType::Prefix.to_string() && trigger.enabled {
                            trace!("🔍 AI SERVICE: Found prefix trigger: {}", trigger.pattern);
                            // No need to use add_trigger_prefix since we removed that field
                        }
                    }
                },
                Err(e) => {
                    error!("🔍 AI SERVICE: Error loading triggers from database: {:?}", e);
                    // Continue anyway - we'll complete initialization
                }
            }
        } else {
            info!("🔍 AI SERVICE: No trigger_repo available, skipping trigger loading");
        }
        
        info!("🔍 AI SERVICE: Database initialization complete");
        Ok(())
    }
    
    /// Internal method to configure a provider without persisting to database
    async fn internal_configure_provider(&self, config: ProviderConfig) -> anyhow::Result<()> {
        info!("🔧 AI SERVICE: internal_configure_provider called for provider: {}", config.provider_type);
        
        match config.provider_type.to_lowercase().as_str() {
            "openai" => {
                info!("🔧 AI SERVICE: Creating OpenAI provider with model: {}", config.default_model);
                let masked_key = if config.api_key.len() > 10 {
                    format!("{}...{}", &config.api_key[0..5], &config.api_key[config.api_key.len()-5..])
                } else {
                    "[API key too short to mask]".to_string()
                };
                info!("🔧 AI SERVICE: Using API key: {}", masked_key);
                
                let provider = OpenAIProvider::new(config.clone());
                info!("🔧 AI SERVICE: Registering provider with AI client");
                self.client.provider().register(provider).await;
                
                // Set this provider as default
                let default_provider = config.provider_type.clone();
                info!("🔧 AI SERVICE: Setting default provider to: {}", default_provider);
                
                // Let's set the enabled flag to true
                let mut enabled = self.enabled.write().await;
                *enabled = true;
                info!("🔧 AI SERVICE: AI service enabled flag set to true");
                
                Ok(())
            },
            "anthropic" => {
                info!("🔧 AI SERVICE: Creating Anthropic provider with model: {}", config.default_model);
                let masked_key = if config.api_key.len() > 10 {
                    format!("{}...{}", &config.api_key[0..5], &config.api_key[config.api_key.len()-5..])
                } else {
                    "[API key too short to mask]".to_string()
                };
                info!("🔧 AI SERVICE: Using API key: {}", masked_key);
                
                let provider = AnthropicProvider::new(config.clone());
                info!("🔧 AI SERVICE: Registering provider with AI client");
                self.client.provider().register(provider).await;
                
                // Set this provider as default
                let default_provider = config.provider_type.clone();
                info!("🔧 AI SERVICE: Setting default provider to: {}", default_provider);
                
                // Let's set the enabled flag to true
                let mut enabled = self.enabled.write().await;
                *enabled = true;
                info!("🔧 AI SERVICE: AI service enabled flag set to true");
                
                Ok(())
            },
            provider_type => {
                let error_msg = format!("Unsupported provider type: {}", provider_type);
                error!("🔧 AI SERVICE: {}", error_msg);
                Err(anyhow!(error_msg))
            },
        }
    }
    
    /// Configure a provider with the given configuration and persist to database
    pub async fn configure_provider(&self, config: ProviderConfig) -> anyhow::Result<()> {
        // First, configure the provider internally
        self.internal_configure_provider(config.clone()).await?;
        
        // If we have repositories, persist the data
        if let (Some(provider_repo), Some(cred_repo), Some(model_repo)) = 
            (&self.provider_repo, &self.ai_credential_repo, &self.model_repo) {
            
            let now = Utc::now();
            
            // Check if provider exists
            let provider_name = config.provider_type.clone();
            let maybe_provider = provider_repo.get_provider_by_name(&provider_name).await?;
            
            let provider_id = if let Some(provider) = maybe_provider {
                provider.provider_id
            } else {
                // Create new provider
                let provider = AiProvider {
                    provider_id: Uuid::new_v4(),
                    name: provider_name.clone(),
                    description: Some(format!("{} provider", provider_name)),
                    enabled: true,
                    created_at: now,
                    updated_at: now,
                };
                provider_repo.create_provider(&provider).await?;
                provider.provider_id
            };
            
            // Create or update credential
            let maybe_default_cred = cred_repo.get_default_credential_for_provider(provider_id).await?;
            
            if let Some(cred) = maybe_default_cred {
                // Update existing credential
                let updated_cred = AiCredential {
                    api_key: config.api_key.clone(),
                    api_base: config.api_base.clone(),
                    updated_at: now,
                    ..cred
                };
                cred_repo.update_credential(&updated_cred).await?;
            } else {
                // Create new credential
                let credential = AiCredential {
                    credential_id: Uuid::new_v4(),
                    provider_id,
                    api_key: config.api_key.clone(),
                    api_base: config.api_base.clone(),
                    is_default: true,
                    additional_data: None,
                    created_at: now,
                    updated_at: now,
                };
                cred_repo.create_credential(&credential).await?;
            }
            
            // Create or update model
            let model_name = config.default_model.clone();
            let maybe_model = model_repo.get_model_by_name(provider_id, &model_name).await?;
            
            if let Some(model) = maybe_model {
                // Update existing model
                let updated_model = AiModel {
                    is_default: true,
                    updated_at: now,
                    ..model
                };
                model_repo.update_model(&updated_model).await?;
                model_repo.set_default_model(updated_model.model_id).await?;
            } else {
                // Create new model
                let model = AiModel {
                    model_id: Uuid::new_v4(),
                    provider_id,
                    name: model_name.clone(),
                    description: Some(format!("{} model", model_name)),
                    is_default: true,
                    capabilities: None,
                    created_at: now,
                    updated_at: now,
                };
                model_repo.create_model(&model).await?;
            }
        }
        
        Ok(())
    }
    
    /// Check if a message should trigger AI processing
    pub async fn should_process_with_ai(&self, message: &str) -> bool {
        trace!("🔍 AI SERVICE: should_process_with_ai called for message: '{}'", message);
        
        // Check if AI is enabled
        let enabled = *self.enabled.read().await;
        if !enabled {
            trace!("🔍 AI SERVICE: AI is disabled, skipping message");
            return false;
        }
        trace!("🔍 AI SERVICE: AI is enabled, checking triggers");
        
        // Normalize the message: trim whitespace and convert to lowercase
        let normalized_message = message.to_lowercase().trim().to_string();
        
        // If we have a trigger repository, check database triggers
        if let Some(trigger_repo) = &self.trigger_repo {
            // Try to fetch all triggers
            match trigger_repo.list_triggers().await {
                Ok(triggers) => {
                    trace!("🔍 AI SERVICE: Checking against {} database triggers", triggers.len());
                    
                    for trigger in triggers {
                        if !trigger.enabled {
                            continue;
                        }
                        
                        match trigger.trigger_type.as_str() {
                            "prefix" => {
                                let prefix = trigger.pattern.to_lowercase();
                                if normalized_message.starts_with(&prefix) {
                                    trace!("🔍 AI SERVICE: Prefix trigger matched: '{}'", trigger.pattern);
                                    return true;
                                }
                                
                                // Also check if the message starts with the prefix with a mention
                                if normalized_message.contains(&prefix) {
                                    let mention_pattern = r"<@!?\d+>";
                                    let re = regex::Regex::new(mention_pattern)
                                        .unwrap_or_else(|_| regex::Regex::new("never match").unwrap());
                                    
                                    if re.is_match(&normalized_message) {
                                        let without_mentions = re.replace_all(&normalized_message, "").trim().to_string();
                                        if without_mentions.starts_with(&prefix) {
                                            trace!("🔍 AI SERVICE: Prefix trigger matched after removing mentions: '{}'", trigger.pattern);
                                            return true;
                                        }
                                    }
                                }
                            },
                            "regex" => {
                                // Use proper regex with error handling
                                match regex::Regex::new(&trigger.pattern) {
                                    Ok(re) => {
                                        if re.is_match(&normalized_message) {
                                            trace!("🔍 AI SERVICE: Regex trigger matched: '{}'", trigger.pattern);
                                            return true;
                                        }
                                    },
                                    Err(e) => {
                                        error!("🔍 AI SERVICE: Invalid regex pattern '{}': {}", trigger.pattern, e);
                                    }
                                }
                            },
                            "mention" => {
                                // Look for mention patterns like <@123456> or @username
                                if normalized_message.contains("<@") || normalized_message.contains("@maow") {
                                    trace!("🔍 AI SERVICE: Mention trigger matched");
                                    return true;
                                }
                            },
                            _ => {
                                // Other trigger types not implemented yet
                            }
                        }
                    }
                },
                Err(e) => {
                    error!("🔍 AI SERVICE: Error fetching triggers: {}", e);
                }
            }
        } else {
            // No database - for testing, accept all messages
            info!("🔍 AI SERVICE: No trigger repository available, accepting all messages");
            return true;
        }
        
        trace!("🔍 AI SERVICE: No trigger matched");
        false
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
        trace!("🔍 AI SERVICE: process_user_message called with user_id: {} and message: '{}'", user_id, message);
        
        // Check for AI providers
        let providers = self.client.provider().get_all().await;
        trace!("🔍 AI SERVICE: Available AI providers: {:?}", providers);
        if providers.is_empty() {
            error!("🔍 AI SERVICE: No AI providers available!");
            return Err(anyhow!("No AI providers configured"));
        }
        
        // Save user message to memory repository if available
        if let Some(memory_repo) = &self.memory_repo {
            let memory = AiMemory {
                memory_id: Uuid::new_v4(),
                user_id,
                platform: "default".to_string(),
                role: MemoryRole::User.to_string(),
                content: message.to_string(),
                timestamp: Utc::now(),
                metadata: None,
            };
            
            if let Err(e) = memory_repo.create_memory(&memory).await {
                error!("🔍 AI SERVICE: Failed to save user message to memory: {:?}", e);
                // Continue even if memory saving fails
            }
        }
        
        // Determine the correct trigger and model/agent to use
        // Note: We're gathering trigger information but not using it yet
        // This will be used in the future to select the right model/agent
        let _model_id: Option<Uuid> = None;
        let _agent_id: Option<Uuid> = None;
        let _system_prompt: Option<String> = None;
        
        if let Some(trigger_repo) = &self.trigger_repo {
            // Try to find matching triggers
            if let Ok(triggers) = trigger_repo.list_triggers_with_details().await {
                for trigger_detail in triggers {
                    let trigger = trigger_detail.trigger;
                    
                    // Skip disabled triggers
                    if !trigger.enabled {
                        continue;
                    }
                    
                    let normalized_message = message.to_lowercase().trim().to_string();
                    let matched = match trigger.trigger_type.as_str() {
                        "prefix" => {
                            let prefix = trigger.pattern.to_lowercase();
                            normalized_message.starts_with(&prefix)
                        },
                        "regex" => {
                            match regex::Regex::new(&trigger.pattern) {
                                Ok(re) => re.is_match(&normalized_message),
                                Err(_) => false,
                            }
                        },
                        "mention" => {
                            normalized_message.contains("@maow") || normalized_message.contains("<@")
                        },
                        _ => false,
                    };
                    
                    if matched {
                        // Record the matching trigger data (future implementation will use this)
                        debug!("Found matching trigger: {}", trigger.trigger_id);
                        // We'll implement model/agent selection in a future update
                        break;
                    }
                }
            }
        }
        
        // Attempt to process with AI
        info!("🔍 AI SERVICE: Calling agent_with_memory");
        let result = match self.client.agent_with_memory(user_id.to_string(), message, 10).await {
            Ok(response) => {
                info!("🔍 AI SERVICE: Successfully generated response: '{}'", response);
                
                // Save assistant response to memory repository if available
                if let Some(memory_repo) = &self.memory_repo {
                    let memory = AiMemory {
                        memory_id: Uuid::new_v4(),
                        user_id,
                        platform: "default".to_string(),
                        role: MemoryRole::Assistant.to_string(),
                        content: response.clone(),
                        timestamp: Utc::now(),
                        metadata: None,
                    };
                    
                    if let Err(e) = memory_repo.create_memory(&memory).await {
                        error!("🔍 AI SERVICE: Failed to save assistant response to memory: {:?}", e);
                        // Continue even if memory saving fails
                    }
                }
                
                Ok(response)
            },
            Err(e) => {
                error!("🔍 AI SERVICE: Failed to generate response: {:?}", e);
                Err(e)
            }
        };
        
        result
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
        
        // Check if we have a prompt repository
        if let Some(prompt_repo) = &self.prompt_repo {
            // Check if a default prompt already exists
            let maybe_default = prompt_repo.get_default_prompt().await?;
            
            if let Some(existing) = maybe_default {
                // Update the existing default prompt
                let updated = AiSystemPrompt {
                    content: prompt.to_string(),
                    updated_at: Utc::now(),
                    ..existing
                };
                prompt_repo.update_prompt(&updated).await?;
            } else {
                // Create a new default prompt
                let new_prompt = AiSystemPrompt {
                    prompt_id: Uuid::new_v4(),
                    name: "Default System Prompt".to_string(),
                    content: prompt.to_string(),
                    description: Some("Default system prompt for AI interactions".to_string()),
                    is_default: true,
                    created_at: Utc::now(),
                    updated_at: Utc::now(),
                };
                prompt_repo.create_prompt(&new_prompt).await?;
            }
        }
        
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
        trace!("Available providers: {:?}", providers);
        
        // Strip trigger prefix from message
        let mut processed_message = message.to_string();
        
        // Get prefixes from repository or use defaults
        let prefixes = self.get_trigger_prefixes().await.unwrap_or_else(|_| {
            vec!["@maowbot".to_string(), "hey maow".to_string()]
        });
        
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
    
    /// Add a trigger prefix to the database
    pub async fn add_trigger_prefix(&self, prefix: &str) -> anyhow::Result<()> {
        if let Some(trigger_repo) = &self.trigger_repo {
            // Check if trigger already exists
            match trigger_repo.get_trigger_by_pattern(prefix).await {
                Ok(Some(_)) => {
                    // Trigger already exists
                    info!("Trigger prefix '{}' already exists", prefix);
                    return Ok(());
                },
                Ok(None) => {
                    // Get default model
                    let mut model_id = None;
                    
                    if let Some(config_repo) = &self.config_repo {
                        if let Ok(Some(config)) = config_repo.get_default_configuration().await {
                            model_id = Some(config.model.model_id);
                        }
                    }
                    
                    // Create new trigger
                    let trigger = AiTrigger {
                        trigger_id: Uuid::new_v4(),
                        trigger_type: TriggerType::Prefix.to_string(),
                        pattern: prefix.to_string(),
                        model_id,
                        agent_id: None,
                        system_prompt: None,
                        platform: None,
                        channel: None,
                        schedule: None,
                        condition: None,
                        enabled: true,
                        created_at: Utc::now(),
                        updated_at: Utc::now(),
                    };
                    
                    trigger_repo.create_trigger(&trigger).await?;
                    info!("Added trigger prefix: {}", prefix);
                },
                Err(e) => {
                    return Err(anyhow!("Error checking for existing trigger: {}", e));
                }
            }
        } else {
            info!("No trigger repository available, cannot add trigger");
        }
        
        Ok(())
    }
    
    /// Remove a trigger prefix from the database
    pub async fn remove_trigger_prefix(&self, prefix: &str) -> anyhow::Result<()> {
        if let Some(trigger_repo) = &self.trigger_repo {
            // Find trigger by pattern
            match trigger_repo.get_trigger_by_pattern(prefix).await {
                Ok(Some(trigger)) => {
                    // Delete the trigger
                    trigger_repo.delete_trigger(trigger.trigger_id).await?;
                    info!("Removed trigger prefix: {}", prefix);
                },
                Ok(None) => {
                    info!("Trigger prefix '{}' not found", prefix);
                },
                Err(e) => {
                    return Err(anyhow!("Error finding trigger: {}", e));
                }
            }
        } else {
            info!("No trigger repository available, cannot remove trigger");
        }
        
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
    
    /// Get the provider repository if available
    pub fn get_provider_repo(&self) -> Option<Arc<dyn AiProviderRepository + Send + Sync>> {
        self.provider_repo.clone()
    }
    
    /// Get the credential repository if available
    pub fn get_ai_credential_repo(&self) -> Option<Arc<dyn AiCredentialRepository + Send + Sync>> {
        self.ai_credential_repo.clone()
    }
    
    /// Get the agent repository if available
    pub fn get_agent_repo(&self) -> Option<Arc<dyn AiAgentRepository + Send + Sync>> {
        self.agent_repo.clone()
    }
    
    /// Get the prompt repository if available
    pub fn get_prompt_repo(&self) -> Option<Arc<dyn AiSystemPromptRepository + Send + Sync>> {
        self.prompt_repo.clone()
    }
    
    /// Get the model repository if available
    pub fn get_model_repo(&self) -> Option<Arc<dyn AiModelRepository + Send + Sync>> {
        self.model_repo.clone()
    }
    
    /// Get the list of trigger prefixes from the database
    pub async fn get_trigger_prefixes(&self) -> anyhow::Result<Vec<String>> {
        let mut prefixes = Vec::new();
        
        if let Some(trigger_repo) = &self.trigger_repo {
            // Get triggers from database
            match trigger_repo.list_triggers().await {
                Ok(triggers) => {
                    for trigger in triggers {
                        if trigger.trigger_type == TriggerType::Prefix.to_string() && trigger.enabled {
                            prefixes.push(trigger.pattern);
                        }
                    }
                },
                Err(e) => {
                    return Err(anyhow!("Error fetching triggers: {}", e));
                }
            }
        } else {
            info!("No trigger repository available, returning default prefixes");
            // Return some default prefixes when no database is available
            prefixes.push("@maowbot".to_string());
            prefixes.push("hey maow".to_string());
        }
        
        Ok(prefixes)
    }
    
    // ------ Agent Management Methods ------
    
    /// Create a new agent in the database
    pub async fn create_agent(
        &self, 
        name: &str, 
        model_id: Uuid, 
        description: Option<&str>,
        system_prompt: Option<&str>,
        capabilities: Option<serde_json::Value>,
    ) -> anyhow::Result<AiAgent> {
        if let Some(agent_repo) = &self.agent_repo {
            let agent = AiAgent::new(
                name,
                description,
                model_id,
                system_prompt,
                capabilities,
                true,
            );
            
            agent_repo.create_agent(&agent).await?;
            info!("Created new agent: {}", name);
            
            Ok(agent)
        } else {
            Err(anyhow!("Agent repository not available"))
        }
    }
    
    /// Get an agent from the database
    pub async fn get_agent(&self, agent_id: Uuid) -> anyhow::Result<Option<AiAgentWithDetails>> {
        if let Some(agent_repo) = &self.agent_repo {
            let agent = agent_repo.get_agent_with_details(agent_id).await?;
            Ok(agent)
        } else {
            Err(anyhow!("Agent repository not available"))
        }
    }
    
    /// List all agents in the database
    pub async fn list_agents(&self) -> anyhow::Result<Vec<AiAgent>> {
        if let Some(agent_repo) = &self.agent_repo {
            let agents = agent_repo.list_agents().await?;
            Ok(agents)
        } else {
            Err(anyhow!("Agent repository not available"))
        }
    }
    
    /// Update an agent in the database
    pub async fn update_agent(&self, agent: &AiAgent) -> anyhow::Result<()> {
        if let Some(agent_repo) = &self.agent_repo {
            agent_repo.update_agent(agent).await?;
            info!("Updated agent: {}", agent.name);
            Ok(())
        } else {
            Err(anyhow!("Agent repository not available"))
        }
    }
    
    /// Delete an agent from the database
    pub async fn delete_agent(&self, agent_id: Uuid) -> anyhow::Result<()> {
        if let Some(agent_repo) = &self.agent_repo {
            agent_repo.delete_agent(agent_id).await?;
            info!("Deleted agent: {}", agent_id);
            Ok(())
        } else {
            Err(anyhow!("Agent repository not available"))
        }
    }
    
    // ------ Action Management Methods ------
    
    /// Create a new action in the database
    pub async fn create_action(
        &self,
        agent_id: Uuid,
        name: &str,
        handler_type: ActionHandlerType, 
        description: Option<&str>,
        input_schema: Option<serde_json::Value>,
        output_schema: Option<serde_json::Value>,
        handler_config: Option<serde_json::Value>,
    ) -> anyhow::Result<AiAction> {
        if let Some(action_repo) = &self.action_repo {
            let action = AiAction::new(
                agent_id,
                name,
                description,
                input_schema,
                output_schema,
                handler_type,
                handler_config,
                true,
            );
            
            action_repo.create_action(&action).await?;
            info!("Created new action: {}", name);
            
            Ok(action)
        } else {
            Err(anyhow!("Action repository not available"))
        }
    }
    
    /// List actions for an agent
    pub async fn list_actions_for_agent(&self, agent_id: Uuid) -> anyhow::Result<Vec<AiAction>> {
        if let Some(action_repo) = &self.action_repo {
            let actions = action_repo.list_actions_for_agent(agent_id).await?;
            Ok(actions)
        } else {
            Err(anyhow!("Action repository not available"))
        }
    }
    
    /// Update an action in the database
    pub async fn update_action(&self, action: &AiAction) -> anyhow::Result<()> {
        if let Some(action_repo) = &self.action_repo {
            action_repo.update_action(action).await?;
            info!("Updated action: {}", action.name);
            Ok(())
        } else {
            Err(anyhow!("Action repository not available"))
        }
    }
    
    /// Delete an action from the database
    pub async fn delete_action(&self, action_id: Uuid) -> anyhow::Result<()> {
        if let Some(action_repo) = &self.action_repo {
            action_repo.delete_action(action_id).await?;
            info!("Deleted action: {}", action_id);
            Ok(())
        } else {
            Err(anyhow!("Action repository not available"))
        }
    }
    
    // ------ Model Management Methods ------
    
    /// Create a new AI model in the database
    pub async fn create_model(
        &self,
        provider_id: Uuid,
        name: &str,
        description: Option<&str>,
        is_default: bool,
        capabilities: Option<serde_json::Value>,
    ) -> anyhow::Result<AiModel> {
        if let Some(model_repo) = &self.model_repo {
            let model = AiModel::new(
                provider_id,
                name,
                description,
                is_default,
                capabilities,
            );
            
            model_repo.create_model(&model).await?;
            info!("Created new model: {}", name);
            
            // If this is the default model, make sure to set it as default
            if is_default {
                model_repo.set_default_model(model.model_id).await?;
            }
            
            Ok(model)
        } else {
            Err(anyhow!("Model repository not available"))
        }
    }
    
    /// List models for a provider
    pub async fn list_models_for_provider(&self, provider_id: Uuid) -> anyhow::Result<Vec<AiModel>> {
        if let Some(model_repo) = &self.model_repo {
            let models = model_repo.list_models_for_provider(provider_id).await?;
            Ok(models)
        } else {
            Err(anyhow!("Model repository not available"))
        }
    }
    
    /// Update a model in the database
    pub async fn update_model(&self, model: &AiModel) -> anyhow::Result<()> {
        if let Some(model_repo) = &self.model_repo {
            model_repo.update_model(model).await?;
            info!("Updated model: {}", model.name);
            
            // If this is the default model, make sure to set it as default
            if model.is_default {
                model_repo.set_default_model(model.model_id).await?;
            }
            
            Ok(())
        } else {
            Err(anyhow!("Model repository not available"))
        }
    }
    
    /// Delete a model from the database
    pub async fn delete_model(&self, model_id: Uuid) -> anyhow::Result<()> {
        if let Some(model_repo) = &self.model_repo {
            model_repo.delete_model(model_id).await?;
            info!("Deleted model: {}", model_id);
            Ok(())
        } else {
            Err(anyhow!("Model repository not available"))
        }
    }
    
    /// Set a model as the default for its provider
    pub async fn set_default_model(&self, model_id: Uuid) -> anyhow::Result<()> {
        if let Some(model_repo) = &self.model_repo {
            model_repo.set_default_model(model_id).await?;
            info!("Set model {} as default", model_id);
            Ok(())
        } else {
            Err(anyhow!("Model repository not available"))
        }
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
        // Verify user exists
        let _user_exists = self.service.user_repo.get(user_id).await
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