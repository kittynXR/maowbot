// AI command adapter for TUI
use maowbot_common_ui::{GrpcClient, commands::ai::AICommands};
use std::collections::HashMap;

pub async fn handle_ai_command(args: &[&str], client: &GrpcClient) -> String {
    if args.is_empty() {
        return "Usage: ai [enable|disable|status|provider|chat|function|prompt]".to_string();
    }

    match args[0] {
        "help" => {
            "AI Command Usage:\n\
             - ai status: Show AI system status\n\
             - ai provider [list|configure|test]: Manage AI providers\n\
             - ai chat <message>: Test chat with AI\n\
             - ai function [list|register|unregister]: Manage AI functions\n\
             - ai prompt [list|set|get]: Manage system prompts".to_string()
        }
        
        "status" => {
            // For now, we'll use the provider list to determine status
            match AICommands::list_providers(client, true).await {
                Ok(result) => {
                    if result.providers.is_empty() {
                        "AI Status: No providers configured".to_string()
                    } else {
                        let active_count = result.providers.iter().filter(|p| p.is_active).count();
                        format!(
                            "AI Status: {} providers configured, {} active\nActive provider: {}",
                            result.providers.len(),
                            active_count,
                            if result.active_provider.is_empty() { "None" } else { &result.active_provider }
                        )
                    }
                }
                Err(e) => format!("Error checking AI status: {}", e),
            }
        }
        
        "provider" => {
            if args.len() < 2 {
                return "Usage: ai provider [list|configure|test]".to_string();
            }
            
            match args[1] {
                "list" => {
                    match AICommands::list_providers(client, false).await {
                        Ok(result) => {
                            if result.providers.is_empty() {
                                "No providers available".to_string()
                            } else {
                                let mut output = "Available providers:\n".to_string();
                                for provider in result.providers {
                                    output.push_str(&format!(
                                        "- {} ({}): {} {}\n",
                                        provider.name,
                                        provider.provider_type,
                                        if provider.is_configured { "Configured" } else { "Not configured" },
                                        if provider.is_active { "[Active]" } else { "" }
                                    ));
                                    if !provider.supported_models.is_empty() {
                                        output.push_str(&format!("  Models: {}\n", provider.supported_models.join(", ")));
                                    }
                                }
                                output
                            }
                        }
                        Err(e) => format!("Error listing providers: {}", e),
                    }
                }
                
                "configure" => {
                    if args.len() < 3 {
                        return "Usage: ai provider configure <provider> --api-key <KEY> [--model <MODEL>] [--api-base <URL>]".to_string();
                    }
                    
                    let provider_name = args[2];
                    let mut config = HashMap::new();
                    
                    // Parse arguments
                    let mut i = 3;
                    while i < args.len() {
                        match args[i] {
                            "--api-key" => {
                                if i + 1 < args.len() {
                                    config.insert("api_key".to_string(), args[i + 1].to_string());
                                    i += 2;
                                } else {
                                    return "Missing value for --api-key".to_string();
                                }
                            }
                            "--model" => {
                                if i + 1 < args.len() {
                                    config.insert("default_model".to_string(), args[i + 1].to_string());
                                    i += 2;
                                } else {
                                    return "Missing value for --model".to_string();
                                }
                            }
                            "--api-base" => {
                                if i + 1 < args.len() {
                                    config.insert("api_base".to_string(), args[i + 1].to_string());
                                    i += 2;
                                } else {
                                    return "Missing value for --api-base".to_string();
                                }
                            }
                            _ => i += 1,
                        }
                    }
                    
                    if !config.contains_key("api_key") {
                        return "API key is required (--api-key <KEY>)".to_string();
                    }
                    
                    match AICommands::configure_provider(client, provider_name, config).await {
                        Ok(result) => result.message,
                        Err(e) => format!("Error configuring provider: {}", e),
                    }
                }
                
                "test" => {
                    if args.len() < 3 {
                        return "Usage: ai provider test <provider> [test message]".to_string();
                    }
                    
                    let provider_name = args[2];
                    let test_prompt = if args.len() > 3 {
                        Some(args[3..].join(" "))
                    } else {
                        None
                    };
                    
                    match AICommands::test_provider(client, provider_name, test_prompt.as_deref()).await {
                        Ok(result) => {
                            if result.success {
                                format!(
                                    "Provider test successful ({}ms):\n{}",
                                    result.latency_ms,
                                    result.response
                                )
                            } else {
                                format!("Provider test failed: {}", result.response)
                            }
                        }
                        Err(e) => format!("Error testing provider: {}", e),
                    }
                }
                
                _ => format!("Unknown provider subcommand: {}", args[1]),
            }
        }
        
        "chat" => {
            if args.len() < 2 {
                return "Usage: ai chat <message>".to_string();
            }
            
            let message = args[1..].join(" ");
            
            match AICommands::generate_chat(client, &message, None).await {
                Ok(result) => {
                    format!(
                        "{}\n\n[Model: {}, Tokens: {}]",
                        result.response,
                        result.model_used,
                        result.tokens_used
                    )
                }
                Err(e) => format!("Error: {}", e),
            }
        }
        
        "function" => {
            if args.len() < 2 {
                return "Usage: ai function [list|register|unregister]".to_string();
            }
            
            match args[1] {
                "list" => {
                    match AICommands::list_functions(client, vec![]).await {
                        Ok(result) => {
                            if result.functions.is_empty() {
                                "No functions registered".to_string()
                            } else {
                                let mut output = "Registered functions:\n".to_string();
                                for func in result.functions {
                                    output.push_str(&format!(
                                        "- {}: {} {} (called {} times)\n",
                                        func.name,
                                        func.description,
                                        if func.is_enabled { "[Enabled]" } else { "[Disabled]" },
                                        func.call_count
                                    ));
                                }
                                output
                            }
                        }
                        Err(e) => format!("Error listing functions: {}", e),
                    }
                }
                
                "register" => {
                    if args.len() < 4 {
                        return "Usage: ai function register <name> <description>".to_string();
                    }
                    
                    let name = args[2];
                    let description = args[3..].join(" ");
                    
                    match AICommands::register_function(client, name, &description).await {
                        Ok(_) => format!("Function '{}' registered successfully", name),
                        Err(e) => format!("Error registering function: {}", e),
                    }
                }
                
                "unregister" => {
                    if args.len() < 3 {
                        return "Usage: ai function unregister <name>".to_string();
                    }
                    
                    let name = args[2];
                    
                    match AICommands::unregister_function(client, name).await {
                        Ok(_) => format!("Function '{}' unregistered successfully", name),
                        Err(e) => format!("Error unregistering function: {}", e),
                    }
                }
                
                _ => format!("Unknown function subcommand: {}", args[1]),
            }
        }
        
        "prompt" => {
            if args.len() < 2 {
                return "Usage: ai prompt [list|set|get]".to_string();
            }
            
            match args[1] {
                "list" => {
                    match AICommands::list_system_prompts(client).await {
                        Ok(result) => {
                            if result.prompts.is_empty() {
                                "No system prompts configured".to_string()
                            } else {
                                let mut output = "System prompts:\n".to_string();
                                for prompt in result.prompts {
                                    let is_active = prompt.prompt_id == result.active_prompt_id;
                                    output.push_str(&format!(
                                        "- {}: {} {}\n",
                                        prompt.prompt_id,
                                        if prompt.prompt.len() > 50 { 
                                            format!("{}...", &prompt.prompt[..50]) 
                                        } else { 
                                            prompt.prompt.clone() 
                                        },
                                        if is_active { "[Active]" } else { "" }
                                    ));
                                }
                                output
                            }
                        }
                        Err(e) => format!("Error listing prompts: {}", e),
                    }
                }
                
                "set" => {
                    if args.len() < 4 {
                        return "Usage: ai prompt set <id> <prompt>".to_string();
                    }
                    
                    let prompt_id = args[2];
                    let prompt = args[3..].join(" ");
                    
                    match AICommands::set_system_prompt(client, prompt_id, &prompt).await {
                        Ok(_) => format!("System prompt '{}' set successfully", prompt_id),
                        Err(e) => format!("Error setting prompt: {}", e),
                    }
                }
                
                "get" => {
                    if args.len() < 3 {
                        return "Usage: ai prompt get <id>".to_string();
                    }
                    
                    let prompt_id = args[2];
                    
                    match AICommands::get_system_prompt(client, prompt_id).await {
                        Ok(prompt) => format!("System prompt '{}':\n{}", prompt_id, prompt),
                        Err(e) => format!("Error getting prompt: {}", e),
                    }
                }
                
                _ => format!("Unknown prompt subcommand: {}", args[1]),
            }
        }
        
        // Note: enable/disable would require additional gRPC methods not currently in the proto
        "enable" => "AI enable/disable is not available through gRPC yet".to_string(),
        "disable" => "AI enable/disable is not available through gRPC yet".to_string(),
        
        _ => format!("Unknown AI subcommand: {}", args[0]),
    }
}