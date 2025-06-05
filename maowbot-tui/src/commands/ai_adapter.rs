use maowbot_common_ui::commands::ai::{AiCommands, AiStatusInfo, ProviderKeyDisplay, ProviderDisplay};
use maowbot_common_ui::grpc_client::GrpcClient;

/// TUI adapter for AI commands
pub struct AiAdapter;

impl AiAdapter {
    /// Handle AI command
    pub async fn handle_command(args: &[&str], grpc_client: &GrpcClient) -> String {
        if args.is_empty() {
            return "Usage: ai [enable|disable|status|provider|configure|chat]".to_string();
        }

        let subcommand = args[0].to_lowercase();
        match subcommand.as_str() {
            "help" => {
                "AI Command Usage:\n\
                 - ai status: Show AI system status\n\
                 - ai enable/disable: Enable or disable AI processing\n\
                 - ai provider [list|show]: Manage AI providers\n\
                 - ai configure [openai|anthropic]: Configure AI providers\n\
                 - ai chat <message>: Test chat with AI".to_string()
            },
            
            "enable" => {
                let mut client = grpc_client.clone();
                match AiCommands::enable(&mut client).await {
                    Ok(message) => message,
                    Err(e) => format!("Error: {}", e)
                }
            },
            
            "disable" => {
                let mut client = grpc_client.clone();
                match AiCommands::disable(&mut client).await {
                    Ok(message) => message,
                    Err(e) => format!("Error: {}", e)
                }
            },
            
            "status" => {
                let mut client = grpc_client.clone();
                match AiCommands::status(&mut client).await {
                    Ok(status) => Self::format_status(status),
                    Err(e) => format!("Error: {}", e)
                }
            },
            
            "provider" => {
                if args.len() < 2 {
                    return "Usage: ai provider [list|show]".to_string();
                }
                
                let provider_command = args[1].to_lowercase();
                match provider_command.as_str() {
                    "list" => {
                        let mut client = grpc_client.clone();
                        match AiCommands::list_providers(&mut client, false).await {
                            Ok(providers) => Self::format_providers(providers),
                            Err(e) => format!("Error: {}", e)
                        }
                    },
                    "show" => {
                        let provider_name = if args.len() > 2 {
                            Some(args[2].to_string())
                        } else {
                            None
                        };
                        
                        let mut client = grpc_client.clone();
                        match AiCommands::show_provider_keys(&mut client, provider_name).await {
                            Ok(keys) => Self::format_provider_keys(keys),
                            Err(e) => format!("Error: {}", e)
                        }
                    },
                    _ => format!("Unknown provider subcommand: {}", provider_command)
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
                let mut model = None;
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
                                model = Some(args[i + 1].to_string());
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
                
                let mut client = grpc_client.clone();
                match AiCommands::configure_provider(&mut client, provider_type, api_key, model, api_base).await {
                    Ok(message) => message,
                    Err(e) => format!("Error: {}", e)
                }
            },
            
            "chat" => {
                if args.len() < 2 {
                    return "Usage: ai chat <MESSAGE>".to_string();
                }
                
                let message = args[1..].join(" ");
                
                let mut client = grpc_client.clone();
                match AiCommands::chat(&mut client, message).await {
                    Ok(response) => response,
                    Err(e) => format!("Error: {}", e)
                }
            },
            
            _ => format!("Unknown AI subcommand: {}", subcommand)
        }
    }
    
    /// Format status information
    fn format_status(status: AiStatusInfo) -> String {
        let mut result = format!("AI Status: {}\n", if status.enabled { "Enabled" } else { "Disabled" });
        result.push_str(&format!("Active Provider: {}\n", status.active_provider));
        result.push_str(&format!("Active Models: {}\n", status.active_models_count));
        result.push_str(&format!("Active Agents: {}\n", status.active_agents_count));
        
        if !status.statistics.is_empty() {
            result.push_str("\nStatistics:\n");
            for (key, value) in &status.statistics {
                result.push_str(&format!("  {}: {}\n", key, value));
            }
        }
        
        result
    }
    
    /// Format provider list
    fn format_providers(providers: Vec<ProviderDisplay>) -> String {
        if providers.is_empty() {
            return "No providers configured".to_string();
        }
        
        let mut result = "Configured providers:\n".to_string();
        for provider in providers {
            result.push_str(&format!("- {} ({}):", provider.name, provider.provider_type));
            if provider.is_active {
                result.push_str(" [ACTIVE]");
            }
            if provider.is_configured {
                result.push_str(" [CONFIGURED]");
            }
            result.push('\n');
            
            if !provider.supported_models.is_empty() {
                result.push_str(&format!("  Models: {}\n", provider.supported_models.join(", ")));
            }
            if !provider.capabilities.is_empty() {
                result.push_str(&format!("  Capabilities: {}\n", provider.capabilities.join(", ")));
            }
        }
        
        result
    }
    
    /// Format provider keys display
    fn format_provider_keys(keys: Vec<ProviderKeyDisplay>) -> String {
        if keys.is_empty() {
            return "No provider API keys configured".to_string();
        }
        
        let mut result = "Provider API Keys:\n".to_string();
        for key in keys {
            result.push_str(&format!("- {}: {}", key.provider_name, key.masked_key));
            if key.is_active {
                result.push_str(" [ACTIVE]");
            }
            if !key.api_base.is_empty() {
                result.push_str(&format!(" (base: {})", key.api_base));
            }
            if let Some(configured_at) = key.configured_at {
                result.push_str(&format!(" - configured at {}", configured_at));
            }
            result.push('\n');
        }
        
        result
    }
}