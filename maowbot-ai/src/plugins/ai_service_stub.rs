use async_trait::async_trait;
use std::collections::HashMap;
use tracing::info;
use uuid::Uuid;
use maowbot_common::error::Error;
use maowbot_common::models::ai::{AiProvider, AiCredential, AiModel, TriggerType, AiTriggerWithDetails, AiTrigger, AiAgent};
use maowbot_common::traits::api::AiApi;
use crate::models::ProviderConfig;

/// A stub implementation of the AiApi trait for development
#[derive(Clone)]
pub struct AiApiStub {}

impl AiApiStub {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl AiApi for AiApiStub {
    /// Get direct access to the AI service
    async fn get_ai_service(&self) -> Result<Option<std::sync::Arc<dyn std::any::Any + Send + Sync>>, Error> {
        // Create a stub AI service that can be properly downcast by the TUI
        info!("🧩 AiApiStub: get_ai_service called, creating stub AiService");
        
        // We need to create a minimal AiService that can be downcast by the TUI
        // This needs to be exported using the EXACT SAME TYPE as is expected by the consumer
        use crate::plugins::ai_service::AiService;
        use maowbot_common::traits::repository_traits::*;
        use std::sync::Arc;
        
        // Create stub repositories
        // First, create an implementation of UserRepo
        struct StubUserRepo;
        #[async_trait]
        impl UserRepo for StubUserRepo {
            async fn get(&self, _user_id: uuid::Uuid) -> Result<Option<maowbot_common::models::user::User>, Error> {
                Ok(None)
            }
            async fn find_by_platform_identity(&self, _platform: &str, _platform_id: &str) -> Result<Option<maowbot_common::models::user::User>, Error> {
                Ok(None)
            }
            async fn create(&self, _user: &maowbot_common::models::user::User) -> Result<(), Error> {
                Ok(())
            }
            async fn update(&self, _user: &maowbot_common::models::user::User) -> Result<(), Error> {
                Ok(())
            }
            async fn list_active_users(&self) -> Result<Vec<maowbot_common::models::user::User>, Error> {
                Ok(vec![])
            }
            async fn get_platforms_for_user(&self, _user_id: uuid::Uuid) -> Result<HashMap<String, String>, Error> {
                Ok(HashMap::new())
            }
            async fn deactivate_user(&self, _user_id: uuid::Uuid) -> Result<(), Error> {
                Ok(())
            }
        }
        
        // Then an implementation of CredentialsRepository
        struct StubCredsRepo;
        #[async_trait]
        impl CredentialsRepository for StubCredsRepo {
            async fn get_credential(&self, _platform: &str, _name: &str) -> Result<Option<maowbot_common::models::credential::Credential>, Error> {
                Ok(None)
            }
            async fn set_credential(&self, _credential: &maowbot_common::models::credential::Credential) -> Result<(), Error> {
                Ok(())
            }
            async fn delete_credential(&self, _platform: &str, _name: &str) -> Result<(), Error> {
                Ok(())
            }
            async fn list_credentials(&self) -> Result<Vec<maowbot_common::models::credential::Credential>, Error> {
                Ok(vec![])
            }
        }
        
        // Create the stub AiService
        let user_repo = Arc::new(StubUserRepo {});
        let creds_repo = Arc::new(StubCredsRepo {});
        
        // Try to create an AiService instance
        info!("🧩 AiApiStub: Creating AiService with stub repositories");
        match AiService::new(user_repo, creds_repo).await {
            Ok(service) => {
                info!("🧩 AiApiStub: Successfully created stub AiService");
                // Set it as enabled
                if let Err(e) = service.set_enabled(true).await {
                    info!("🧩 AiApiStub: Failed to enable stub service: {:?}", e);
                } else {
                    info!("🧩 AiApiStub: Enabled stub service");
                }
                
                // Wrap it in an Arc and return it with EXACTLY the same type as expected by consumers 
                let service_arc = Arc::new(service);
                
                // The type needs to match exactly what's used in the downcast operation
                Ok(Some(service_arc as Arc<dyn std::any::Any + Send + Sync>))
            },
            Err(e) => {
                info!("🧩 AiApiStub: Failed to create stub AiService: {:?}", e);
                Ok(None)
            }
        }
    }
    
    /// Generate a chat completion
    async fn generate_chat(&self, _messages: Vec<serde_json::Value>) -> Result<String, Error> {
        Ok("This is a stub response from AiApiStub".to_string())
    }
    
    /// Generate a completion with function calling
    async fn generate_with_functions(&self, _messages: Vec<serde_json::Value>) -> Result<serde_json::Value, Error> {
        Ok(serde_json::json!({
            "content": "This is a stub response from AiApiStub",
            "function_call": null
        }))
    }
    
    /// Process a user message with context
    async fn process_user_message(&self, _user_id: Uuid, _message: &str) -> Result<String, Error> {
        Ok("This is a stub response from AiApiStub".to_string())
    }
    
    /// Register a new function
    async fn register_ai_function(&self, name: &str, _description: &str) -> Result<(), Error> {
        info!("Stub: Registering function '{}'", name);
        Ok(())
    }
    
    /// Unregister a function
    async fn unregister_ai_function(&self, name: &str) -> Result<(), Error> {
        info!("Stub: Unregistering function '{}'", name);
        Ok(())
    }
    
    /// List all registered functions
    async fn list_ai_functions(&self) -> Result<Vec<(String, String)>, Error> {
        Ok(vec![
            ("function1".to_string(), "Description 1".to_string()),
            ("function2".to_string(), "Description 2".to_string())
        ])
    }
    
    /// Set the system prompt
    async fn set_system_prompt(&self, prompt: &str) -> Result<(), Error> {
        info!("Stub: Setting system prompt to '{}'", prompt);
        Ok(())
    }
    
    /// Configure an AI provider with the given configuration
    async fn configure_ai_provider(&self, config: serde_json::Value) -> Result<(), Error> {
        info!("Stub: Configuring AI provider: {:?}", config);
        
        // Extracting information from the configuration
        let provider_type = config["provider_type"].as_str().unwrap_or("unknown");
        let api_key = config["api_key"].as_str().unwrap_or("stub-api-key");
        let model = config["default_model"].as_str().unwrap_or("unknown-model");
        
        info!("Would configure provider {} with model {}", provider_type, model);
        info!("API key detected (masked): {}***", &api_key[0..3]);
        
        Ok(())
    }
    
    /// Enable or disable the AI service
    async fn enable_ai_service(&self, enabled: bool) -> Result<(), Error> {
        info!("Stub: Setting AI service enabled to {}", enabled);
        Ok(())
    }
    
    /// Get the status of the AI service
    async fn get_ai_status(&self) -> Result<String, Error> {
        Ok("Enabled: true, Provider: Stub".to_string())
    }
    
    /// Add a new AI provider
    async fn add_ai_provider(&self, name: &str, _description: Option<&str>) -> Result<Uuid, Error> {
        let id = Uuid::new_v4();
        info!("Stub: Adding AI provider '{}' with ID {}", name, id);
        Ok(id)
    }
    
    /// List all AI providers
    async fn list_ai_providers(&self) -> Result<Vec<AiProvider>, Error> {
        Ok(vec![
            AiProvider {
                provider_id: Uuid::new_v4(),
                name: "OpenAI".to_string(),
                description: Some("GPT models".to_string()),
                enabled: true,
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
            },
            AiProvider {
                provider_id: Uuid::new_v4(),
                name: "Anthropic".to_string(),
                description: Some("Claude models".to_string()),
                enabled: true,
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
            },
        ])
    }
    
    /// Delete an AI provider by ID
    async fn delete_ai_provider_by_id(&self, provider_id: Uuid) -> Result<(), Error> {
        info!("Stub: Deleting AI provider with ID {}", provider_id);
        Ok(())
    }
    
    /// Delete an AI provider by name
    async fn delete_ai_provider_by_name(&self, name: &str) -> Result<(), Error> {
        info!("Stub: Deleting AI provider '{}'", name);
        Ok(())
    }
    
    /// Update an AI provider by ID
    async fn update_ai_provider_by_id(&self, provider_id: Uuid, enabled: bool, _description: Option<&str>) -> Result<(), Error> {
        info!("Stub: Updating AI provider with ID {} (enabled: {})", provider_id, enabled);
        Ok(())
    }
    
    /// Update an AI provider by name
    async fn update_ai_provider_by_name(&self, name: &str, enabled: bool, _description: Option<&str>) -> Result<(), Error> {
        info!("Stub: Updating AI provider '{}' (enabled: {})", name, enabled);
        Ok(())
    }
    
    /// Add a new AI credential
    async fn add_ai_credential(&self, provider: &str, _api_key: &str, _api_base: Option<&str>) -> Result<Uuid, Error> {
        let id = Uuid::new_v4();
        info!("Stub: Adding AI credential for provider '{}' with ID {}", provider, id);
        Ok(id)
    }
    
    /// List all AI credentials for a provider
    async fn list_ai_credentials(&self, provider: &str) -> Result<Vec<AiCredential>, Error> {
        let provider_id = Uuid::new_v4();
        Ok(vec![
            AiCredential {
                credential_id: Uuid::new_v4(),
                provider_id,
                api_key: "sk-xxxx".to_string(),
                api_base: None,
                is_default: true,
                additional_data: None,
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
            }
        ])
    }
    
    /// Delete an AI credential
    async fn delete_ai_credential(&self, credential_id: Uuid) -> Result<(), Error> {
        info!("Stub: Deleting AI credential with ID {}", credential_id);
        Ok(())
    }
    
    /// Update an AI credential
    async fn update_ai_credential(&self, credential_id: Uuid, _api_key: &str, _api_base: Option<&str>) -> Result<(), Error> {
        info!("Stub: Updating AI credential with ID {}", credential_id);
        Ok(())
    }
    
    /// Set a credential as the default for its provider
    async fn set_default_ai_credential(&self, credential_id: Uuid) -> Result<(), Error> {
        info!("Stub: Setting AI credential with ID {} as default", credential_id);
        Ok(())
    }
    
    /// Add a new AI model
    async fn add_ai_model(&self, provider: &str, name: &str, _description: Option<&str>) -> Result<Uuid, Error> {
        let id = Uuid::new_v4();
        info!("Stub: Adding AI model '{}' for provider '{}' with ID {}", name, provider, id);
        Ok(id)
    }
    
    /// List all AI models for a provider
    async fn list_ai_models(&self, provider: &str) -> Result<Vec<AiModel>, Error> {
        let provider_id = Uuid::new_v4();
        Ok(vec![
            AiModel {
                model_id: Uuid::new_v4(),
                provider_id,
                name: "gpt-4".to_string(),
                description: Some("GPT-4 model".to_string()),
                is_default: true,
                capabilities: None,
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
            }
        ])
    }
    
    /// Delete an AI model
    async fn delete_ai_model(&self, model_id: Uuid) -> Result<(), Error> {
        info!("Stub: Deleting AI model with ID {}", model_id);
        Ok(())
    }
    
    /// Update an AI model
    async fn update_ai_model(&self, model_id: Uuid, name: &str, _description: Option<&str>) -> Result<(), Error> {
        info!("Stub: Updating AI model with ID {} to name '{}'", model_id, name);
        Ok(())
    }
    
    /// Set a model as the default for its provider
    async fn set_default_ai_model(&self, model_id: Uuid) -> Result<(), Error> {
        info!("Stub: Setting AI model with ID {} as default", model_id);
        Ok(())
    }
    
    /// Add a new AI trigger
    async fn add_ai_trigger(&self, trigger_type: TriggerType, pattern: &str, model: &str, _system_prompt: Option<&str>) -> Result<Uuid, Error> {
        let id = Uuid::new_v4();
        info!("Stub: Adding AI trigger pattern '{}' with type {:?} for model '{}' with ID {}", 
             pattern, trigger_type, model, id);
        Ok(id)
    }
    
    /// List all AI triggers
    async fn list_ai_triggers(&self) -> Result<Vec<AiTriggerWithDetails>, Error> {
        let provider_id = Uuid::new_v4();
        let model_id = Uuid::new_v4();
        Ok(vec![
            AiTriggerWithDetails {
                trigger: AiTrigger {
                    trigger_id: Uuid::new_v4(),
                    trigger_type: "prefix".to_string(),
                    pattern: "hey maow".to_string(),
                    model_id: Some(model_id),
                    agent_id: None,
                    system_prompt: Some("You are Maow, a helpful AI assistant.".to_string()),
                    platform: None,
                    channel: None,
                    schedule: None,
                    condition: None,
                    enabled: true,
                    created_at: chrono::Utc::now(),
                    updated_at: chrono::Utc::now(),
                },
                model: Some(AiModel {
                    model_id,
                    provider_id,
                    name: "gpt-4".to_string(),
                    description: Some("GPT-4 model".to_string()),
                    is_default: true,
                    capabilities: None,
                    created_at: chrono::Utc::now(),
                    updated_at: chrono::Utc::now(),
                }),
                provider: Some(AiProvider {
                    provider_id,
                    name: "OpenAI".to_string(),
                    description: Some("GPT models".to_string()),
                    enabled: true,
                    created_at: chrono::Utc::now(),
                    updated_at: chrono::Utc::now(),
                }),
                agent: None,
            }
        ])
    }
    
    /// Delete an AI trigger by ID
    async fn delete_ai_trigger_by_id(&self, trigger_id: Uuid) -> Result<(), Error> {
        info!("Stub: Deleting AI trigger with ID {}", trigger_id);
        Ok(())
    }
    
    /// Delete an AI trigger by pattern
    async fn delete_ai_trigger_by_pattern(&self, pattern: &str) -> Result<(), Error> {
        info!("Stub: Deleting AI trigger with pattern '{}'", pattern);
        Ok(())
    }
    
    /// Update an AI trigger
    async fn update_ai_trigger(&self, trigger_id: Uuid, enabled: bool, model: &str, _system_prompt: Option<&str>) -> Result<(), Error> {
        info!("Stub: Updating AI trigger with ID {} (enabled: {}, model: '{}')", 
             trigger_id, enabled, model);
        Ok(())
    }
}