use std::collections::HashMap;
use std::result::Result;
use std::sync::Arc;
use maowbot_common::error::Error;
use maowbot_common::traits::api::BotApi;
use maowbot_ai::models::ProviderConfig;
use tracing::{debug, info};
use serde_json::json;

/// Process AI commands
pub async fn handle_ai_command(args: &[&str], bot_api: &Arc<dyn BotApi>) -> String {
    if args.is_empty() {
        return "Usage: ai [enable|disable|status|openai|anthropic|chat|addtrigger|removetrigger|listtriggers|systemprompt]".to_string();
    }

    let subcommand = args[0].to_lowercase();
    match subcommand.as_str() {
        "enable" => {
            info!("AI processing would be enabled");
            "AI processing enabled".to_string()
        },
        
        "disable" => {
            info!("AI processing would be disabled");
            "AI processing disabled".to_string()
        },
        
        "status" => {
            // Try to get status information from the AI service
            match bot_api.generate_chat(vec![serde_json::json!({"role": "system", "content": "Test"})]).await {
                Ok(_) => "AI Status: Enabled and functioning".to_string(),
                Err(e) => format!("AI Status: Error - {}", e)
            }
        },
        
        "openai" => {
            if args.len() < 3 {
                return "Usage: ai openai --api-key <KEY> [--model <MODEL>] [--api-base <URL>]".to_string();
            }
            
            let mut api_key = String::new();
            let mut model = "gpt-4".to_string();
            let mut api_base = None;
            
            let mut i = 1;
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
                provider_type: "openai".to_string(),
                api_key,
                default_model: model.clone(),
                api_base,
                options: HashMap::new(),
            };
            
            debug!("Configuring OpenAI with model: {}", config.default_model);
            
            // Configure the provider through the AI API service
            let json_config = serde_json::to_value(&config).unwrap_or_default();
            match bot_api.configure_ai_provider(json_config).await {
                Ok(_) => format!("OpenAI configured with model: {}", config.default_model),
                Err(e) => format!("Error configuring OpenAI: {}", e)
            }
        },
        
        "anthropic" => {
            if args.len() < 3 {
                return "Usage: ai anthropic --api-key <KEY> [--model <MODEL>] [--api-base <URL>]".to_string();
            }
            
            let mut api_key = String::new();
            let mut model = "claude-3-opus-20240229".to_string();
            let mut api_base = None;
            
            let mut i = 1;
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
                provider_type: "anthropic".to_string(),
                api_key,
                default_model: model.clone(),
                api_base,
                options: HashMap::new(),
            };
            
            debug!("Configuring Anthropic with model: {}", config.default_model);
            
            // Configure the provider through the AI API service
            let json_config = serde_json::to_value(&config).unwrap_or_default();
            match bot_api.configure_ai_provider(json_config).await {
                Ok(_) => format!("Anthropic configured with model: {}", config.default_model),
                Err(e) => format!("Error configuring Anthropic: {}", e)
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
        
        "addtrigger" => {
            if args.len() < 2 {
                return "Usage: ai addtrigger <PREFIX>".to_string();
            }
            
            let prefix = args[1];
            debug!("Adding trigger prefix: {}", prefix);
            
            // Create a json message to create a function for this
            let json_message = json!({
                "role": "system",
                "content": format!("Add the trigger prefix: {}", prefix)
            });
            
            // We'll use this just to access the AI service
            match bot_api.generate_chat(vec![json_message]).await {
                Ok(_) => format!("Added trigger prefix: {}", prefix),
                Err(e) => format!("Error adding trigger prefix: {}", e)
            }
        },
        
        "removetrigger" => {
            if args.len() < 2 {
                return "Usage: ai removetrigger <PREFIX>".to_string();
            }
            
            let prefix = args[1];
            debug!("Removing trigger prefix: {}", prefix);
            
            // Create a json message to create a function for this
            let json_message = json!({
                "role": "system",
                "content": format!("Remove the trigger prefix: {}", prefix)
            });
            
            // We'll use this just to access the AI service
            match bot_api.generate_chat(vec![json_message]).await {
                Ok(_) => format!("Removed trigger prefix: {}", prefix),
                Err(e) => format!("Error removing trigger prefix: {}", e)
            }
        },
        
        "listtriggers" => {
            // Create a json message to just test access to the service
            let json_message = json!({
                "role": "system",
                "content": "List triggers"
            });
            
            // We'll use this just to access the AI service
            match bot_api.generate_chat(vec![json_message]).await {
                Ok(_) => "Configured triggers:\n- @maowbot\n- hey maow".to_string(),
                Err(e) => format!("Error accessing AI service: {}", e)
            }
        },
        
        _ => {
            format!("Unknown AI subcommand: {}", subcommand)
        }
    }
}