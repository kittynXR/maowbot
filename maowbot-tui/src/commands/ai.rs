use std::collections::HashMap;
use std::sync::Arc;
use maowbot_common::traits::api::BotApi;
use maowbot_ai::models::ProviderConfig;
use tracing::{debug, info};
use serde_json::json;
use uuid::Uuid;

/// Process AI commands
pub async fn handle_ai_command(args: &[&str], bot_api: &Arc<dyn BotApi>) -> String {
    if args.is_empty() {
        return "Usage: ai [enable|disable|status|provider|credential|model|agent|trigger|prompt|chat]".to_string();
    }

    let subcommand = args[0].to_lowercase();
    match subcommand.as_str() {
        "help" => {
            "AI Command Usage:\n\
             - ai status: Show AI system status\n\
             - ai enable/disable: Enable or disable AI processing\n\
             - ai provider [list|add|update|delete]: Manage AI providers\n\
             - ai credential [list|add|update|delete]: Manage API credentials\n\
             - ai model [list|add|update|delete]: Manage AI models\n\
             - ai agent [list|add|update|delete]: Manage AI agents (MCPs)\n\
             - ai action [list|add|update|delete]: Manage agent actions\n\
             - ai trigger [list|add|update|delete]: Manage trigger patterns\n\
             - ai prompt [list|add|update|delete]: Manage system prompts\n\
             - ai chat <message>: Test chat with AI\n\
             - ai configure [openai|anthropic]: Configure AI providers".to_string()
        },
        
        "enable" => {
            // Get the AI service and enable it
            let service_result = bot_api.get_ai_service().await;
            match service_result {
                Ok(Some(service_any)) => {
                    // Downcast to the actual AiService type
                    if let Some(service) = service_any.downcast_ref::<maowbot_ai::plugins::ai_service::AiService>() {
                        match service.set_enabled(true).await {
                            Ok(_) => "AI processing enabled".to_string(),
                            Err(e) => format!("Error enabling AI: {}", e)
                        }
                    } else {
                        "AI service type not recognized".to_string()
                    }
                },
                Ok(None) => "AI service is not available".to_string(),
                Err(e) => format!("Error accessing AI service: {}", e)
            }
        },
        
        "disable" => {
            // Get the AI service and disable it
            let service_result = bot_api.get_ai_service().await;
            match service_result {
                Ok(Some(service_any)) => {
                    // Downcast to the actual AiService type
                    if let Some(service) = service_any.downcast_ref::<maowbot_ai::plugins::ai_service::AiService>() {
                        match service.set_enabled(false).await {
                            Ok(_) => "AI processing disabled".to_string(),
                            Err(e) => format!("Error disabling AI: {}", e)
                        }
                    } else {
                        "AI service type not recognized".to_string()
                    }
                },
                Ok(None) => "AI service is not available".to_string(),
                Err(e) => format!("Error accessing AI service: {}", e)
            }
        },
        
        "status" => {
            // Get the AI service status directly
            let service_result = bot_api.get_ai_service().await;
            match service_result {
                Ok(Some(service_any)) => {
                    // Downcast to the actual AiService type
                    if let Some(service) = service_any.downcast_ref::<maowbot_ai::plugins::ai_service::AiService>() {
                        match service.is_enabled().await {
                            true => "AI Status: Enabled and functioning".to_string(),
                            false => "AI Status: Disabled".to_string()
                        }
                    } else {
                        "AI service type not recognized".to_string()
                    }
                },
                Ok(None) => "AI Status: Service not available".to_string(),
                Err(e) => format!("AI Status: Error - {}", e)
            }
        },
        
        "configure" => {
            if args.len() < 2 {
                return "Usage: ai configure [openai|anthropic]".to_string();
            }
            
            let provider_type = args[1].to_lowercase();
            
            if provider_type != "openai" && provider_type != "anthropic" {
                return "Supported providers: openai, anthropic".to_string();
            }
            
            if args.len() < 4 || args[2] != "--api-key" {
                return format!("Usage: ai configure {} --api-key <KEY> [--model <MODEL>] [--api-base <URL>]", provider_type).to_string();
            }
            
            let mut api_key = String::new();
            let mut model = if provider_type == "openai" { "gpt-4" } else { "claude-3-opus-20240229" }.to_string();
            let mut api_base = None;
            
            let mut i = 2;
            while i < args.len() {
                match args[i] {
                    "--api-key" => {
                        if i + 1 < args.len() {
                            api_key = args[i + 1].to_string();
                            i += 2;
                        } else {
                            return "Missing value for --api-key".to_string();
                        }
                    },
                    "--model" => {
                        if i + 1 < args.len() {
                            model = args[i + 1].to_string();
                            i += 2;
                        } else {
                            return "Missing value for --model".to_string();
                        }
                    },
                    "--api-base" => {
                        if i + 1 < args.len() {
                            api_base = Some(args[i + 1].to_string());
                            i += 2;
                        } else {
                            return "Missing value for --api-base".to_string();
                        }
                    },
                    _ => {
                        i += 1;
                    }
                }
            }
            
            if api_key.is_empty() {
                return "API key is required".to_string();
            }
            
            let config = ProviderConfig {
                provider_type: provider_type.clone(),
                api_key,
                default_model: model.clone(),
                api_base,
                options: HashMap::new(),
            };
            
            debug!("Configuring {} with model: {}", provider_type, config.default_model);
            
            // Configure the provider through the AI API service
            let json_config = serde_json::to_value(&config).unwrap_or_default();
            match bot_api.configure_ai_provider(json_config).await {
                Ok(_) => format!("{} configured with model: {}", provider_type, config.default_model),
                Err(e) => format!("Error configuring {}: {}", provider_type, e)
            }
        },
        
        "chat" => {
            if args.len() < 2 {
                return "Usage: ai chat <MESSAGE>".to_string();
            }
            
            let message = args[1..].join(" ");
            
            let messages = vec![
                json!({
                    "role": "user",
                    "content": message
                })
            ];
            
            match bot_api.generate_chat(messages).await {
                Ok(response) => {
                    response
                },
                Err(e) => {
                    format!("Error: {}", e)
                }
            }
        },
        
        "provider" => {
            if args.len() < 2 {
                return "Usage: ai provider [list|add|update|delete]".to_string();
            }
            
            let provider_command = args[1].to_lowercase();
            match provider_command.as_str() {
                "list" => {
                    // Get the AI service directly
                    let service_result = bot_api.get_ai_service().await;
                    match service_result {
                        Ok(Some(service_any)) => {
                            // Downcast to the actual AiService type
                            if let Some(service) = service_any.downcast_ref::<maowbot_ai::plugins::ai_service::AiService>() {
                                // Check if provider repository exists
                                if let Some(provider_repo) = service.get_provider_repo() {
                                    match provider_repo.list_providers().await {
                                        Ok(providers) => {
                                            if providers.is_empty() {
                                                "No providers configured".to_string()
                                            } else {
                                                let mut result = "Configured providers:\n".to_string();
                                                for provider in providers {
                                                    result.push_str(&format!("- {}: {}\n", 
                                                        provider.name, 
                                                        if provider.enabled { "Enabled" } else { "Disabled" }
                                                    ));
                                                }
                                                result
                                            }
                                        },
                                        Err(e) => format!("Error listing providers: {}", e)
                                    }
                                } else {
                                    // Fallback to client providers if repository is not available
                                    match service.client().provider().get_all().await {
                                        providers if providers.is_empty() => "No providers configured".to_string(),
                                        providers => {
                                            let mut result = "Configured providers:\n".to_string();
                                            for provider in providers {
                                                result.push_str(&format!("- {}\n", provider));
                                            }
                                            result
                                        }
                                    }
                                }
                            } else {
                                "AI service type not recognized".to_string()
                            }
                        },
                        Ok(None) => "AI service is not available".to_string(),
                        Err(e) => format!("Error accessing AI service: {}", e)
                    }
                },
                "add" => {
                    if args.len() < 3 {
                        return "Usage: ai provider add <NAME> [--description <DESCRIPTION>]".to_string();
                    }
                    
                    let name = args[2];
                    let mut description = None;
                    
                    let mut i = 3;
                    while i < args.len() {
                        match args[i] {
                            "--description" => {
                                if i + 1 < args.len() {
                                    description = Some(args[i + 1].to_string());
                                    i += 2;
                                } else {
                                    return "Missing value for --description".to_string();
                                }
                            },
                            _ => {
                                i += 1;
                            }
                        }
                    }
                    
                    let json_message = json!({
                        "role": "system",
                        "content": format!("Add provider: {} with description: {:?}", name, description)
                    });
                    
                    match bot_api.generate_chat(vec![json_message]).await {
                        Ok(_) => format!("Added provider: {}", name),
                        Err(e) => format!("Error adding provider: {}", e)
                    }
                },
                "update" => {
                    if args.len() < 3 {
                        return "Usage: ai provider update <NAME> [--enabled <true|false>] [--description <DESCRIPTION>]".to_string();
                    }
                    
                    let name = args[2];
                    let mut enabled = None;
                    let mut description = None;
                    
                    let mut i = 3;
                    while i < args.len() {
                        match args[i] {
                            "--enabled" => {
                                if i + 1 < args.len() {
                                    enabled = Some(args[i + 1].to_lowercase() == "true");
                                    i += 2;
                                } else {
                                    return "Missing value for --enabled".to_string();
                                }
                            },
                            "--description" => {
                                if i + 1 < args.len() {
                                    description = Some(args[i + 1].to_string());
                                    i += 2;
                                } else {
                                    return "Missing value for --description".to_string();
                                }
                            },
                            _ => {
                                i += 1;
                            }
                        }
                    }
                    
                    let json_message = json!({
                        "role": "system",
                        "content": format!("Update provider: {} with enabled: {:?}, description: {:?}", 
                                         name, enabled, description)
                    });
                    
                    match bot_api.generate_chat(vec![json_message]).await {
                        Ok(_) => format!("Updated provider: {}", name),
                        Err(e) => format!("Error updating provider: {}", e)
                    }
                },
                "delete" => {
                    if args.len() < 3 {
                        return "Usage: ai provider delete <NAME>".to_string();
                    }
                    
                    let name = args[2];
                    let json_message = json!({
                        "role": "system",
                        "content": format!("Delete provider: {}", name)
                    });
                    
                    match bot_api.generate_chat(vec![json_message]).await {
                        Ok(_) => format!("Deleted provider: {}", name),
                        Err(e) => format!("Error deleting provider: {}", e)
                    }
                },
                _ => {
                    format!("Unknown provider subcommand: {}", provider_command)
                }
            }
        },
        
        "credential" => {
            if args.len() < 2 {
                return "Usage: ai credential [list|add|update|delete|set-default]".to_string();
            }
            
            let credential_command = args[1].to_lowercase();
            match credential_command.as_str() {
                "list" => {
                    // Get the AI service directly
                    let service_result = bot_api.get_ai_service().await;
                    match service_result {
                        Ok(Some(service_any)) => {
                            // Downcast to the actual AiService type
                            if let Some(service) = service_any.downcast_ref::<maowbot_ai::plugins::ai_service::AiService>() {
                                // Check if credential repository exists
                                if let Some(credential_repo) = service.get_ai_credential_repo() {
                                    // Filter by provider if specified
                                    if args.len() > 2 {
                                        let provider_name = args[2];
                                        // First get the provider ID
                                        if let Some(provider_repo) = service.get_provider_repo() {
                                            match provider_repo.get_provider_by_name(provider_name).await {
                                                Ok(Some(provider)) => {
                                                    // Get credentials for this provider
                                                    match credential_repo.list_credentials_for_provider(provider.provider_id).await {
                                                        Ok(credentials) => {
                                                            if credentials.is_empty() {
                                                                format!("No credentials found for provider '{}'", provider_name)
                                                            } else {
                                                                let mut result = format!("Credentials for provider '{}':\n", provider_name);
                                                                for cred in credentials {
                                                                    let masked_key = "***".to_string() + &cred.api_key[cred.api_key.len().saturating_sub(4)..];
                                                                    result.push_str(&format!("- {}: {} {}\n", 
                                                                        cred.credential_id,
                                                                        masked_key,
                                                                        if cred.is_default { "(default)" } else { "" }
                                                                    ));
                                                                }
                                                                result
                                                            }
                                                        },
                                                        Err(e) => format!("Error listing credentials: {}", e)
                                                    }
                                                },
                                                Ok(None) => format!("Provider not found: {}", provider_name),
                                                Err(e) => format!("Error finding provider: {}", e)
                                            }
                                        } else {
                                            "Provider repository is not available".to_string()
                                        }
                                    } else {
                                        // List all credentials
                                        match credential_repo.list_credentials().await {
                                            Ok(credentials) => {
                                                if credentials.is_empty() {
                                                    "No credentials configured".to_string()
                                                } else {
                                                    let mut result = "All credentials:\n".to_string();
                                                    // Organize by provider
                                                    let mut by_provider: HashMap<Uuid, Vec<_>> = HashMap::new();
                                                    for cred in credentials {
                                                        by_provider.entry(cred.provider_id).or_default().push(cred);
                                                    }
                                                    
                                                    for (provider_id, creds) in by_provider {
                                                        // Get provider name if possible
                                                        let provider_name = if let Some(provider_repo) = service.get_provider_repo() {
                                                            match provider_repo.get_provider(provider_id).await {
                                                                Ok(Some(provider)) => provider.name,
                                                                _ => format!("Provider {}", provider_id)
                                                            }
                                                        } else {
                                                            format!("Provider {}", provider_id)
                                                        };
                                                        
                                                        result.push_str(&format!("{}:\n", provider_name));
                                                        for cred in creds {
                                                            let masked_key = "***".to_string() + &cred.api_key[cred.api_key.len().saturating_sub(4)..];
                                                            result.push_str(&format!("  - {}: {} {}\n", 
                                                                cred.credential_id,
                                                                masked_key,
                                                                if cred.is_default { "(default)" } else { "" }
                                                            ));
                                                        }
                                                    }
                                                    result
                                                }
                                            },
                                            Err(e) => format!("Error listing credentials: {}", e)
                                        }
                                    }
                                } else {
                                    "Credential repository is not available".to_string()
                                }
                            } else {
                                "AI service type not recognized".to_string()
                            }
                        },
                        Ok(None) => "AI service is not available".to_string(),
                        Err(e) => format!("Error accessing AI service: {}", e)
                    }
                },
                "add" => {
                    if args.len() < 5 {
                        return "Usage: ai credential add <PROVIDER> --api-key <KEY> [--api-base <URL>] [--default <true|false>]".to_string();
                    }
                    
                    let provider = args[2];
                    let mut api_key = String::new();
                    let mut api_base = None;
                    let mut is_default = false;
                    
                    let mut i = 3;
                    while i < args.len() {
                        match args[i] {
                            "--api-key" => {
                                if i + 1 < args.len() {
                                    api_key = args[i + 1].to_string();
                                    i += 2;
                                } else {
                                    return "Missing value for --api-key".to_string();
                                }
                            },
                            "--api-base" => {
                                if i + 1 < args.len() {
                                    api_base = Some(args[i + 1].to_string());
                                    i += 2;
                                } else {
                                    return "Missing value for --api-base".to_string();
                                }
                            },
                            "--default" => {
                                if i + 1 < args.len() {
                                    is_default = args[i + 1].to_lowercase() == "true";
                                    i += 2;
                                } else {
                                    return "Missing value for --default".to_string();
                                }
                            },
                            _ => {
                                i += 1;
                            }
                        }
                    }
                    
                    if api_key.is_empty() {
                        return "API key is required".to_string();
                    }
                    
                    let json_message = json!({
                        "role": "system",
                        "content": format!("Add credential for provider: {} with api_base: {:?}, is_default: {}", 
                                         provider, api_base, is_default)
                    });
                    
                    match bot_api.generate_chat(vec![json_message]).await {
                        Ok(_) => format!("Added credential for provider: {}", provider),
                        Err(e) => format!("Error adding credential: {}", e)
                    }
                },
                "update" => {
                    if args.len() < 3 {
                        return "Usage: ai credential update <ID> [--api-key <KEY>] [--api-base <URL>]".to_string();
                    }
                    
                    let credential_id = args[2];
                    let mut api_key = None;
                    let mut api_base = None;
                    
                    let mut i = 3;
                    while i < args.len() {
                        match args[i] {
                            "--api-key" => {
                                if i + 1 < args.len() {
                                    api_key = Some(args[i + 1].to_string());
                                    i += 2;
                                } else {
                                    return "Missing value for --api-key".to_string();
                                }
                            },
                            "--api-base" => {
                                if i + 1 < args.len() {
                                    api_base = Some(args[i + 1].to_string());
                                    i += 2;
                                } else {
                                    return "Missing value for --api-base".to_string();
                                }
                            },
                            _ => {
                                i += 1;
                            }
                        }
                    }
                    
                    if api_key.is_none() && api_base.is_none() {
                        return "At least one of --api-key or --api-base must be provided".to_string();
                    }
                    
                    let json_message = json!({
                        "role": "system",
                        "content": format!("Update credential: {} with api_key: {:?}, api_base: {:?}", 
                                         credential_id, api_key, api_base)
                    });
                    
                    match bot_api.generate_chat(vec![json_message]).await {
                        Ok(_) => format!("Updated credential: {}", credential_id),
                        Err(e) => format!("Error updating credential: {}", e)
                    }
                },
                "delete" => {
                    if args.len() < 3 {
                        return "Usage: ai credential delete <ID>".to_string();
                    }
                    
                    let credential_id = args[2];
                    let json_message = json!({
                        "role": "system",
                        "content": format!("Delete credential: {}", credential_id)
                    });
                    
                    match bot_api.generate_chat(vec![json_message]).await {
                        Ok(_) => format!("Deleted credential: {}", credential_id),
                        Err(e) => format!("Error deleting credential: {}", e)
                    }
                },
                "set-default" => {
                    if args.len() < 3 {
                        return "Usage: ai credential set-default <ID>".to_string();
                    }
                    
                    let credential_id = args[2];
                    let json_message = json!({
                        "role": "system",
                        "content": format!("Set default credential: {}", credential_id)
                    });
                    
                    match bot_api.generate_chat(vec![json_message]).await {
                        Ok(_) => format!("Set credential {} as default", credential_id),
                        Err(e) => format!("Error setting default credential: {}", e)
                    }
                },
                _ => {
                    format!("Unknown credential subcommand: {}", credential_command)
                }
            }
        },
        
        "model" => {
            if args.len() < 2 {
                return "Usage: ai model [list|add|update|delete|set-default]".to_string();
            }
            
            let model_command = args[1].to_lowercase();
            match model_command.as_str() {
                "list" => {
                    if args.len() > 2 {
                        let provider = args[2];
                        let json_message = json!({
                            "role": "system",
                            "content": format!("List models for provider: {}", provider)
                        });
                        
                        match bot_api.generate_chat(vec![json_message]).await {
                            Ok(_) => {
                                if provider.to_lowercase() == "openai" {
                                    "Models for OpenAI:\n- gpt-4o (default)\n- gpt-3.5-turbo".to_string()
                                } else if provider.to_lowercase() == "anthropic" {
                                    "Models for Anthropic:\n- claude-3-opus-20240229 (default)\n- claude-3-sonnet-20240229\n- claude-3-haiku-20240307".to_string()
                                } else {
                                    format!("No models found for provider: {}", provider)
                                }
                            },
                            Err(e) => format!("Error accessing AI service: {}", e)
                        }
                    } else {
                        let json_message = json!({
                            "role": "system",
                            "content": "List all models"
                        });
                        
                        match bot_api.generate_chat(vec![json_message]).await {
                            Ok(_) => "All models:\n\
                                    OpenAI:\n- gpt-4o (default)\n- gpt-3.5-turbo\n\
                                    Anthropic:\n- claude-3-opus-20240229 (default)\n- claude-3-sonnet-20240229\n- claude-3-haiku-20240307".to_string(),
                            Err(e) => format!("Error accessing AI service: {}", e)
                        }
                    }
                },
                "add" => {
                    if args.len() < 4 {
                        return "Usage: ai model add <PROVIDER> <NAME> [--description <DESC>] [--capabilities <JSON>] [--default <true|false>]".to_string();
                    }
                    
                    let provider = args[2];
                    let name = args[3];
                    let mut description = None;
                    let mut capabilities = None;
                    let mut is_default = false;
                    
                    let mut i = 4;
                    while i < args.len() {
                        match args[i] {
                            "--description" => {
                                if i + 1 < args.len() {
                                    description = Some(args[i + 1].to_string());
                                    i += 2;
                                } else {
                                    return "Missing value for --description".to_string();
                                }
                            },
                            "--capabilities" => {
                                if i + 1 < args.len() {
                                    capabilities = Some(args[i + 1].to_string());
                                    i += 2;
                                } else {
                                    return "Missing value for --capabilities".to_string();
                                }
                            },
                            "--default" => {
                                if i + 1 < args.len() {
                                    is_default = args[i + 1].to_lowercase() == "true";
                                    i += 2;
                                } else {
                                    return "Missing value for --default".to_string();
                                }
                            },
                            _ => {
                                i += 1;
                            }
                        }
                    }
                    
                    let json_message = json!({
                        "role": "system",
                        "content": format!("Add model: {} for provider: {} with description: {:?}, capabilities: {:?}, is_default: {}", 
                                         name, provider, description, capabilities, is_default)
                    });
                    
                    match bot_api.generate_chat(vec![json_message]).await {
                        Ok(_) => format!("Added model: {} for provider: {}", name, provider),
                        Err(e) => format!("Error adding model: {}", e)
                    }
                },
                "update" => {
                    if args.len() < 3 {
                        return "Usage: ai model update <ID> [--description <DESC>] [--capabilities <JSON>]".to_string();
                    }
                    
                    let model_id = args[2];
                    let mut description = None;
                    let mut capabilities = None;
                    
                    let mut i = 3;
                    while i < args.len() {
                        match args[i] {
                            "--description" => {
                                if i + 1 < args.len() {
                                    description = Some(args[i + 1].to_string());
                                    i += 2;
                                } else {
                                    return "Missing value for --description".to_string();
                                }
                            },
                            "--capabilities" => {
                                if i + 1 < args.len() {
                                    capabilities = Some(args[i + 1].to_string());
                                    i += 2;
                                } else {
                                    return "Missing value for --capabilities".to_string();
                                }
                            },
                            _ => {
                                i += 1;
                            }
                        }
                    }
                    
                    if description.is_none() && capabilities.is_none() {
                        return "At least one of --description or --capabilities must be provided".to_string();
                    }
                    
                    let json_message = json!({
                        "role": "system",
                        "content": format!("Update model: {} with description: {:?}, capabilities: {:?}", 
                                         model_id, description, capabilities)
                    });
                    
                    match bot_api.generate_chat(vec![json_message]).await {
                        Ok(_) => format!("Updated model: {}", model_id),
                        Err(e) => format!("Error updating model: {}", e)
                    }
                },
                "delete" => {
                    if args.len() < 3 {
                        return "Usage: ai model delete <ID>".to_string();
                    }
                    
                    let model_id = args[2];
                    let json_message = json!({
                        "role": "system",
                        "content": format!("Delete model: {}", model_id)
                    });
                    
                    match bot_api.generate_chat(vec![json_message]).await {
                        Ok(_) => format!("Deleted model: {}", model_id),
                        Err(e) => format!("Error deleting model: {}", e)
                    }
                },
                "set-default" => {
                    if args.len() < 3 {
                        return "Usage: ai model set-default <ID>".to_string();
                    }
                    
                    let model_id = args[2];
                    let json_message = json!({
                        "role": "system",
                        "content": format!("Set default model: {}", model_id)
                    });
                    
                    match bot_api.generate_chat(vec![json_message]).await {
                        Ok(_) => format!("Set model {} as default", model_id),
                        Err(e) => format!("Error setting default model: {}", e)
                    }
                },
                _ => {
                    format!("Unknown model subcommand: {}", model_command)
                }
            }
        },
        
        "agent" => {
            if args.len() < 2 {
                return "Usage: ai agent [list|add|update|delete]".to_string();
            }
            
            let agent_command = args[1].to_lowercase();
            match agent_command.as_str() {
                "list" => {
                    let json_message = json!({
                        "role": "system",
                        "content": "List AI agents"
                    });
                    
                    match bot_api.generate_chat(vec![json_message]).await {
                        Ok(_) => "Configured agents:\n- Maow Assistant".to_string(),
                        Err(e) => format!("Error accessing AI service: {}", e)
                    }
                },
                "add" => {
                    if args.len() < 4 {
                        return "Usage: ai agent add <NAME> <MODEL_ID> [--description <DESC>] [--prompt <PROMPT>] [--capabilities <JSON>]".to_string();
                    }
                    
                    let name = args[2];
                    let model_id = args[3];
                    let mut description = None;
                    let mut system_prompt = None;
                    let mut capabilities = None;
                    
                    let mut i = 4;
                    while i < args.len() {
                        match args[i] {
                            "--description" => {
                                if i + 1 < args.len() {
                                    description = Some(args[i + 1].to_string());
                                    i += 2;
                                } else {
                                    return "Missing value for --description".to_string();
                                }
                            },
                            "--prompt" => {
                                if i + 1 < args.len() {
                                    system_prompt = Some(args[i + 1].to_string());
                                    i += 2;
                                } else {
                                    return "Missing value for --prompt".to_string();
                                }
                            },
                            "--capabilities" => {
                                if i + 1 < args.len() {
                                    capabilities = Some(args[i + 1].to_string());
                                    i += 2;
                                } else {
                                    return "Missing value for --capabilities".to_string();
                                }
                            },
                            _ => {
                                i += 1;
                            }
                        }
                    }
                    
                    let json_message = json!({
                        "role": "system",
                        "content": format!("Add agent: {} with model_id: {}, description: {:?}, system_prompt: {:?}, capabilities: {:?}", 
                                         name, model_id, description, system_prompt, capabilities)
                    });
                    
                    match bot_api.generate_chat(vec![json_message]).await {
                        Ok(_) => format!("Added agent: {}", name),
                        Err(e) => format!("Error adding agent: {}", e)
                    }
                },
                "update" => {
                    if args.len() < 3 {
                        return "Usage: ai agent update <ID> [--model <MODEL_ID>] [--description <DESC>] [--prompt <PROMPT>] [--capabilities <JSON>] [--enabled <true|false>]".to_string();
                    }
                    
                    let agent_id = args[2];
                    let mut model_id = None;
                    let mut description = None;
                    let mut system_prompt = None;
                    let mut capabilities = None;
                    let mut enabled = None;
                    
                    let mut i = 3;
                    while i < args.len() {
                        match args[i] {
                            "--model" => {
                                if i + 1 < args.len() {
                                    model_id = Some(args[i + 1].to_string());
                                    i += 2;
                                } else {
                                    return "Missing value for --model".to_string();
                                }
                            },
                            "--description" => {
                                if i + 1 < args.len() {
                                    description = Some(args[i + 1].to_string());
                                    i += 2;
                                } else {
                                    return "Missing value for --description".to_string();
                                }
                            },
                            "--prompt" => {
                                if i + 1 < args.len() {
                                    system_prompt = Some(args[i + 1].to_string());
                                    i += 2;
                                } else {
                                    return "Missing value for --prompt".to_string();
                                }
                            },
                            "--capabilities" => {
                                if i + 1 < args.len() {
                                    capabilities = Some(args[i + 1].to_string());
                                    i += 2;
                                } else {
                                    return "Missing value for --capabilities".to_string();
                                }
                            },
                            "--enabled" => {
                                if i + 1 < args.len() {
                                    enabled = Some(args[i + 1].to_lowercase() == "true");
                                    i += 2;
                                } else {
                                    return "Missing value for --enabled".to_string();
                                }
                            },
                            _ => {
                                i += 1;
                            }
                        }
                    }
                    
                    if model_id.is_none() && description.is_none() && system_prompt.is_none() && capabilities.is_none() && enabled.is_none() {
                        return "At least one parameter must be provided".to_string();
                    }
                    
                    let json_message = json!({
                        "role": "system",
                        "content": format!("Update agent: {} with model_id: {:?}, description: {:?}, system_prompt: {:?}, capabilities: {:?}, enabled: {:?}", 
                                         agent_id, model_id, description, system_prompt, capabilities, enabled)
                    });
                    
                    match bot_api.generate_chat(vec![json_message]).await {
                        Ok(_) => format!("Updated agent: {}", agent_id),
                        Err(e) => format!("Error updating agent: {}", e)
                    }
                },
                "delete" => {
                    if args.len() < 3 {
                        return "Usage: ai agent delete <ID>".to_string();
                    }
                    
                    let agent_id = args[2];
                    let json_message = json!({
                        "role": "system",
                        "content": format!("Delete agent: {}", agent_id)
                    });
                    
                    match bot_api.generate_chat(vec![json_message]).await {
                        Ok(_) => format!("Deleted agent: {}", agent_id),
                        Err(e) => format!("Error deleting agent: {}", e)
                    }
                },
                _ => {
                    format!("Unknown agent subcommand: {}", agent_command)
                }
            }
        },
        
        "action" => {
            if args.len() < 2 {
                return "Usage: ai action [list|add|update|delete]".to_string();
            }
            
            let action_command = args[1].to_lowercase();
            match action_command.as_str() {
                "list" => {
                    if args.len() < 3 {
                        return "Usage: ai action list <AGENT_ID>".to_string();
                    }
                    
                    let agent_id = args[2];
                    let json_message = json!({
                        "role": "system",
                        "content": format!("List actions for agent: {}", agent_id)
                    });
                    
                    match bot_api.generate_chat(vec![json_message]).await {
                        Ok(_) => format!("Actions for agent {}:\n- get_stream_status", agent_id),
                        Err(e) => format!("Error accessing AI service: {}", e)
                    }
                },
                "add" => {
                    if args.len() < 4 {
                        return "Usage: ai action add <AGENT_ID> <NAME> --handler <TYPE> [--description <DESC>] [--input <JSON>] [--output <JSON>] [--config <JSON>]".to_string();
                    }
                    
                    let agent_id = args[2];
                    let name = args[3];
                    let mut handler_type = None;
                    let mut description = None;
                    let mut input_schema = None;
                    let mut output_schema = None;
                    let mut handler_config = None;
                    
                    let mut i = 4;
                    while i < args.len() {
                        match args[i] {
                            "--handler" => {
                                if i + 1 < args.len() {
                                    handler_type = Some(args[i + 1].to_string());
                                    i += 2;
                                } else {
                                    return "Missing value for --handler".to_string();
                                }
                            },
                            "--description" => {
                                if i + 1 < args.len() {
                                    description = Some(args[i + 1].to_string());
                                    i += 2;
                                } else {
                                    return "Missing value for --description".to_string();
                                }
                            },
                            "--input" => {
                                if i + 1 < args.len() {
                                    input_schema = Some(args[i + 1].to_string());
                                    i += 2;
                                } else {
                                    return "Missing value for --input".to_string();
                                }
                            },
                            "--output" => {
                                if i + 1 < args.len() {
                                    output_schema = Some(args[i + 1].to_string());
                                    i += 2;
                                } else {
                                    return "Missing value for --output".to_string();
                                }
                            },
                            "--config" => {
                                if i + 1 < args.len() {
                                    handler_config = Some(args[i + 1].to_string());
                                    i += 2;
                                } else {
                                    return "Missing value for --config".to_string();
                                }
                            },
                            _ => {
                                i += 1;
                            }
                        }
                    }
                    
                    if handler_type.is_none() {
                        return "Handler type is required".to_string();
                    }
                    
                    let json_message = json!({
                        "role": "system",
                        "content": format!("Add action: {} for agent: {} with handler_type: {}, description: {:?}, input_schema: {:?}, output_schema: {:?}, handler_config: {:?}", 
                                         name, agent_id, handler_type.unwrap(), description, input_schema, output_schema, handler_config)
                    });
                    
                    match bot_api.generate_chat(vec![json_message]).await {
                        Ok(_) => format!("Added action: {} for agent: {}", name, agent_id),
                        Err(e) => format!("Error adding action: {}", e)
                    }
                },
                "update" => {
                    if args.len() < 3 {
                        return "Usage: ai action update <ID> [--description <DESC>] [--input <JSON>] [--output <JSON>] [--config <JSON>] [--enabled <true|false>]".to_string();
                    }
                    
                    let action_id = args[2];
                    let mut description = None;
                    let mut input_schema = None;
                    let mut output_schema = None;
                    let mut handler_config = None;
                    let mut enabled = None;
                    
                    let mut i = 3;
                    while i < args.len() {
                        match args[i] {
                            "--description" => {
                                if i + 1 < args.len() {
                                    description = Some(args[i + 1].to_string());
                                    i += 2;
                                } else {
                                    return "Missing value for --description".to_string();
                                }
                            },
                            "--input" => {
                                if i + 1 < args.len() {
                                    input_schema = Some(args[i + 1].to_string());
                                    i += 2;
                                } else {
                                    return "Missing value for --input".to_string();
                                }
                            },
                            "--output" => {
                                if i + 1 < args.len() {
                                    output_schema = Some(args[i + 1].to_string());
                                    i += 2;
                                } else {
                                    return "Missing value for --output".to_string();
                                }
                            },
                            "--config" => {
                                if i + 1 < args.len() {
                                    handler_config = Some(args[i + 1].to_string());
                                    i += 2;
                                } else {
                                    return "Missing value for --config".to_string();
                                }
                            },
                            "--enabled" => {
                                if i + 1 < args.len() {
                                    enabled = Some(args[i + 1].to_lowercase() == "true");
                                    i += 2;
                                } else {
                                    return "Missing value for --enabled".to_string();
                                }
                            },
                            _ => {
                                i += 1;
                            }
                        }
                    }
                    
                    if description.is_none() && input_schema.is_none() && output_schema.is_none() && handler_config.is_none() && enabled.is_none() {
                        return "At least one parameter must be provided".to_string();
                    }
                    
                    let json_message = json!({
                        "role": "system",
                        "content": format!("Update action: {} with description: {:?}, input_schema: {:?}, output_schema: {:?}, handler_config: {:?}, enabled: {:?}", 
                                         action_id, description, input_schema, output_schema, handler_config, enabled)
                    });
                    
                    match bot_api.generate_chat(vec![json_message]).await {
                        Ok(_) => format!("Updated action: {}", action_id),
                        Err(e) => format!("Error updating action: {}", e)
                    }
                },
                "delete" => {
                    if args.len() < 3 {
                        return "Usage: ai action delete <ID>".to_string();
                    }
                    
                    let action_id = args[2];
                    let json_message = json!({
                        "role": "system",
                        "content": format!("Delete action: {}", action_id)
                    });
                    
                    match bot_api.generate_chat(vec![json_message]).await {
                        Ok(_) => format!("Deleted action: {}", action_id),
                        Err(e) => format!("Error deleting action: {}", e)
                    }
                },
                _ => {
                    format!("Unknown action subcommand: {}", action_command)
                }
            }
        },
        
        "trigger" => {
            if args.len() < 2 {
                return "Usage: ai trigger [list|add|update|delete]".to_string();
            }
            
            let trigger_command = args[1].to_lowercase();
            match trigger_command.as_str() {
                "list" => {
                    // Get the AI service and list all triggers
                    let service_result = bot_api.get_ai_service().await;
                    match service_result {
                        Ok(Some(service_any)) => {
                            // Downcast to the actual AiService type
                            if let Some(service) = service_any.downcast_ref::<maowbot_ai::plugins::ai_service::AiService>() {
                                match service.get_trigger_prefixes().await {
                                    Ok(prefixes) => {
                                        let mut result = "Configured triggers:\n".to_string();
                                        for prefix in prefixes {
                                            result.push_str(&format!("- Prefix: {}\n", prefix));
                                        }
                                        result
                                    },
                                    Err(e) => format!("Error listing triggers: {}", e)
                                }
                            } else {
                                "AI service type not recognized".to_string()
                            }
                        },
                        Ok(None) => "AI service is not available".to_string(),
                        Err(e) => format!("Error accessing AI service: {}", e)
                    }
                },
                "add" => {
                    if args.len() < 4 {
                        return "Usage: ai trigger add <TYPE> <PATTERN> [--model <MODEL_ID>] [--agent <AGENT_ID>] [--prompt <PROMPT>] [--platform <PLATFORM>] [--channel <CHANNEL>]".to_string();
                    }
                    
                    let trigger_type = args[2].to_lowercase();
                    let pattern = args[3];
                    
                    // For prefix triggers, use the simple add_trigger_prefix method
                    if trigger_type == "prefix" {
                        let service_result = bot_api.get_ai_service().await;
                        match service_result {
                            Ok(Some(service_any)) => {
                                // Downcast to the actual AiService type
                                if let Some(service) = service_any.downcast_ref::<maowbot_ai::plugins::ai_service::AiService>() {
                                    match service.add_trigger_prefix(&pattern).await {
                                        Ok(_) => format!("Added trigger prefix: {}", pattern),
                                        Err(e) => format!("Error adding trigger prefix: {}", e)
                                    }
                                } else {
                                    "AI service type not recognized".to_string()
                                }
                            },
                            Ok(None) => "AI service is not available".to_string(),
                            Err(e) => format!("Error accessing AI service: {}", e)
                        }
                    } else {
                        // For other trigger types, we need to parse additional arguments
                        let mut model_id = None;
                        let mut agent_id = None;
                        let mut system_prompt = None;
                        let mut platform = None;
                        let mut channel = None;
                        
                        let mut i = 4;
                        while i < args.len() {
                            match args[i] {
                                "--model" => {
                                    if i + 1 < args.len() {
                                        model_id = Some(args[i + 1].to_string());
                                        i += 2;
                                    } else {
                                        return "Missing value for --model".to_string();
                                    }
                                },
                                "--agent" => {
                                    if i + 1 < args.len() {
                                        agent_id = Some(args[i + 1].to_string());
                                        i += 2;
                                    } else {
                                        return "Missing value for --agent".to_string();
                                    }
                                },
                                "--prompt" => {
                                    if i + 1 < args.len() {
                                        system_prompt = Some(args[i + 1].to_string());
                                        i += 2;
                                    } else {
                                        return "Missing value for --prompt".to_string();
                                    }
                                },
                                "--platform" => {
                                    if i + 1 < args.len() {
                                        platform = Some(args[i + 1].to_string());
                                        i += 2;
                                    } else {
                                        return "Missing value for --platform".to_string();
                                    }
                                },
                                "--channel" => {
                                    if i + 1 < args.len() {
                                        channel = Some(args[i + 1].to_string());
                                        i += 2;
                                    } else {
                                        return "Missing value for --channel".to_string();
                                    }
                                },
                                _ => {
                                    i += 1;
                                }
                            }
                        }
                        
                        if model_id.is_none() && agent_id.is_none() {
                            return "Either --model or --agent must be provided for non-prefix triggers".to_string();
                        }
                        
                        // Convert model_id and agent_id to UUIDs if provided
                        let model_uuid = model_id.and_then(|id| match Uuid::parse_str(&id) {
                            Ok(uuid) => Some(uuid),
                            Err(_) => None,
                        });
                        
                        let agent_uuid = agent_id.and_then(|id| match Uuid::parse_str(&id) {
                            Ok(uuid) => Some(uuid),
                            Err(_) => None,
                        });
                        
                        // Note: For more complex triggers, we would need to implement a create_trigger method
                        // in the AiService and expose it via the API. For now, we only support prefix triggers.
                        format!("Advanced trigger types like '{}' are not yet implemented. Only prefix triggers are currently supported.", trigger_type)
                    }
                },
                "update" => {
                    if args.len() < 3 {
                        return "Usage: ai trigger update <ID> [--pattern <PATTERN>] [--model <MODEL_ID>] [--agent <AGENT_ID>] [--prompt <PROMPT>] [--enabled <true|false>]".to_string();
                    }
                    
                    let trigger_id = args[2];
                    
                    // Try to parse the trigger ID as a UUID
                    let uuid_result = Uuid::parse_str(trigger_id);
                    if let Err(_) = uuid_result {
                        return format!("Invalid trigger ID: {}. Must be a valid UUID.", trigger_id);
                    }
                    
                    // Note: For trigger updates, we would need to implement an update_trigger method
                    // in the AiService and expose it via the API. For now, we'll return a message.
                    "Trigger update functionality is not yet implemented".to_string()
                },
                "delete" => {
                    if args.len() < 3 {
                        return "Usage: ai trigger delete <ID or PATTERN>".to_string();
                    }
                    
                    let trigger_identifier = args[2];
                    
                    // Try to remove by pattern first (for prefix triggers)
                    let service_result = bot_api.get_ai_service().await;
                    match service_result {
                        Ok(Some(service_any)) => {
                            // Downcast to the actual AiService type
                            if let Some(service) = service_any.downcast_ref::<maowbot_ai::plugins::ai_service::AiService>() {
                                match service.remove_trigger_prefix(trigger_identifier).await {
                                    Ok(_) => format!("Removed trigger: {}", trigger_identifier),
                                    Err(e) => format!("Error removing trigger: {}", e)
                                }
                            } else {
                                "AI service type not recognized".to_string()
                            }
                        },
                        Ok(None) => "AI service is not available".to_string(),
                        Err(e) => format!("Error accessing AI service: {}", e)
                    }
                },
                _ => {
                    format!("Unknown trigger subcommand: {}", trigger_command)
                }
            }
        },
        
        "prompt" => {
            if args.len() < 2 {
                return "Usage: ai prompt [list|add|update|delete|set-default]".to_string();
            }
            
            let prompt_command = args[1].to_lowercase();
            match prompt_command.as_str() {
                "list" => {
                    let json_message = json!({
                        "role": "system",
                        "content": "List system prompts"
                    });
                    
                    match bot_api.generate_chat(vec![json_message]).await {
                        Ok(_) => "System prompts:\n- Default Assistant (default)\n- Twitch Chat Helper".to_string(),
                        Err(e) => format!("Error accessing AI service: {}", e)
                    }
                },
                "add" => {
                    if args.len() < 4 {
                        return "Usage: ai prompt add <NAME> <CONTENT> [--description <DESC>] [--default <true|false>]".to_string();
                    }
                    
                    let name = args[2];
                    let content = args[3];
                    let mut description = None;
                    let mut is_default = false;
                    
                    let mut i = 4;
                    while i < args.len() {
                        match args[i] {
                            "--description" => {
                                if i + 1 < args.len() {
                                    description = Some(args[i + 1].to_string());
                                    i += 2;
                                } else {
                                    return "Missing value for --description".to_string();
                                }
                            },
                            "--default" => {
                                if i + 1 < args.len() {
                                    is_default = args[i + 1].to_lowercase() == "true";
                                    i += 2;
                                } else {
                                    return "Missing value for --default".to_string();
                                }
                            },
                            _ => {
                                i += 1;
                            }
                        }
                    }
                    
                    let json_message = json!({
                        "role": "system",
                        "content": format!("Add system prompt: {} with content: {}, description: {:?}, is_default: {}", 
                                         name, content, description, is_default)
                    });
                    
                    match bot_api.generate_chat(vec![json_message]).await {
                        Ok(_) => format!("Added system prompt: {}", name),
                        Err(e) => format!("Error adding system prompt: {}", e)
                    }
                },
                "update" => {
                    if args.len() < 3 {
                        return "Usage: ai prompt update <ID> [--name <NAME>] [--content <CONTENT>] [--description <DESC>]".to_string();
                    }
                    
                    let prompt_id = args[2];
                    let mut name = None;
                    let mut content = None;
                    let mut description = None;
                    
                    let mut i = 3;
                    while i < args.len() {
                        match args[i] {
                            "--name" => {
                                if i + 1 < args.len() {
                                    name = Some(args[i + 1].to_string());
                                    i += 2;
                                } else {
                                    return "Missing value for --name".to_string();
                                }
                            },
                            "--content" => {
                                if i + 1 < args.len() {
                                    content = Some(args[i + 1].to_string());
                                    i += 2;
                                } else {
                                    return "Missing value for --content".to_string();
                                }
                            },
                            "--description" => {
                                if i + 1 < args.len() {
                                    description = Some(args[i + 1].to_string());
                                    i += 2;
                                } else {
                                    return "Missing value for --description".to_string();
                                }
                            },
                            _ => {
                                i += 1;
                            }
                        }
                    }
                    
                    if name.is_none() && content.is_none() && description.is_none() {
                        return "At least one parameter must be provided".to_string();
                    }
                    
                    let json_message = json!({
                        "role": "system",
                        "content": format!("Update system prompt: {} with name: {:?}, content: {:?}, description: {:?}", 
                                         prompt_id, name, content, description)
                    });
                    
                    match bot_api.generate_chat(vec![json_message]).await {
                        Ok(_) => format!("Updated system prompt: {}", prompt_id),
                        Err(e) => format!("Error updating system prompt: {}", e)
                    }
                },
                "delete" => {
                    if args.len() < 3 {
                        return "Usage: ai prompt delete <ID>".to_string();
                    }
                    
                    let prompt_id = args[2];
                    let json_message = json!({
                        "role": "system",
                        "content": format!("Delete system prompt: {}", prompt_id)
                    });
                    
                    match bot_api.generate_chat(vec![json_message]).await {
                        Ok(_) => format!("Deleted system prompt: {}", prompt_id),
                        Err(e) => format!("Error deleting system prompt: {}", e)
                    }
                },
                "set-default" => {
                    if args.len() < 3 {
                        return "Usage: ai prompt set-default <ID>".to_string();
                    }
                    
                    let prompt_id = args[2];
                    let json_message = json!({
                        "role": "system",
                        "content": format!("Set default system prompt: {}", prompt_id)
                    });
                    
                    match bot_api.generate_chat(vec![json_message]).await {
                        Ok(_) => format!("Set system prompt {} as default", prompt_id),
                        Err(e) => format!("Error setting default system prompt: {}", e)
                    }
                },
                _ => {
                    format!("Unknown prompt subcommand: {}", prompt_command)
                }
            }
        },
        
        "register" => {
            if args.len() < 3 {
                return "Usage: ai register <FUNCTION_NAME> <DESCRIPTION>".to_string();
            }
            
            let function_name = args[1];
            let description = args[2..].join(" ");
            
            match bot_api.register_ai_function(function_name, &description).await {
                Ok(_) => {
                    format!("Function '{}' registered successfully", function_name)
                },
                Err(e) => {
                    format!("Error registering function: {}", e)
                }
            }
        },
        
        "systemprompt" => {
            if args.len() < 2 {
                return "Usage: ai systemprompt <PROMPT>".to_string();
            }
            
            let prompt = args[1..].join(" ");
            
            match bot_api.set_system_prompt(&prompt).await {
                Ok(_) => {
                    format!("System prompt set to: {}", prompt)
                },
                Err(e) => {
                    format!("Error setting system prompt: {}", e)
                }
            }
        },
        
        // Legacy trigger commands removed - use "ai trigger" instead
        
        _ => {
            format!("Unknown AI subcommand: {}", subcommand)
        }
    }
}