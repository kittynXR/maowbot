// Connection command adapter for TUI - consolidates start/stop/autostart/chat
use maowbot_common_ui::{GrpcClient, commands::connectivity::{ConnectivityCommands, AutostartConfig}};
use crate::tui_module_simple::SimpleTuiModule;
use std::sync::Arc;
use maowbot_proto::maowbot::services::{
    GetConfigRequest, ListActiveRuntimesRequest,
};

pub async fn handle_connection_command(
    args: &[&str], 
    client: &GrpcClient, 
    tui_module: &Arc<SimpleTuiModule>
) -> String {
    if args.is_empty() {
        return "Usage: connection <start|stop|autostart|chat|status> [options]".to_string();
    }

    match args[0] {
        "start" => {
            if args.len() < 2 {
                return "Usage: connection start <platform> [account]".to_string();
            }
            let platform = args[1];
            let account = args.get(2).map(|s| *s).unwrap_or("");
            
            if account.is_empty() {
                // List available accounts
                match ConnectivityCommands::list_platform_accounts(client, platform).await {
                    Ok(accounts) => {
                        if accounts.is_empty() {
                            return format!("No accounts found for platform {}", platform);
                        }
                        return format!("Please specify an account. Available accounts for {}:\n{}", 
                            platform,
                            accounts.iter()
                                .map(|a| format!("  - {}", a.user_name))
                                .collect::<Vec<_>>()
                                .join("\n")
                        );
                    }
                    Err(e) => return format!("Error listing accounts: {}", e),
                }
            }
            
            match ConnectivityCommands::start_platform(client, platform, account).await {
                Ok(result) => format!("Started {} runtime for account {}", result.platform, result.account),
                Err(e) => format!("Error starting platform: {}", e),
            }
        }
        
        "stop" => {
            if args.len() < 2 {
                return "Usage: connection stop <platform> [account]".to_string();
            }
            let platform = args[1];
            let account = args.get(2).map(|s| *s).unwrap_or("");
            
            match ConnectivityCommands::stop_platform(client, platform, account).await {
                Ok(result) => format!("Stopped {} runtime for account {}", result.platform, result.account),
                Err(e) => format!("Error stopping platform: {}", e),
            }
        }
        
        "autostart" => {
            if args.len() < 2 {
                return "Usage: connection autostart <on|off|list> [platform] [account]".to_string();
            }
            
            match args[1] {
                "on" => {
                    if args.len() < 4 {
                        return "Usage: connection autostart on <platform> <account>".to_string();
                    }
                    let platform = args[2];
                    let account = args[3];
                    
                    match ConnectivityCommands::configure_autostart(client, true, platform, account).await {
                        Ok(result) => format!("Autostart enabled for {} on {}", 
                            result.account, result.platform
                        ),
                        Err(e) => format!("Error enabling autostart: {}", e),
                    }
                }
                
                "off" => {
                    if args.len() < 4 {
                        return "Usage: connection autostart off <platform> <account>".to_string();
                    }
                    let platform = args[2];
                    let account = args[3];
                    
                    match ConnectivityCommands::configure_autostart(client, false, platform, account).await {
                        Ok(result) => format!("Autostart disabled for {} on {}", 
                            result.account, result.platform
                        ),
                        Err(e) => format!("Error disabling autostart: {}", e),
                    }
                }
                
                "list" => {
                    // Get autostart config
                    let get_request = GetConfigRequest {
                        key: "autostart".to_string(),
                        include_metadata: false,
                    };
                    
                    let mut config_client = client.config.clone();
                    match config_client.get_config(get_request).await {
                        Ok(response) => {
                            let config_value = response.into_inner()
                                .config
                                .map(|c| c.value)
                                .unwrap_or_default();
                                
                            if config_value.is_empty() {
                                return "No autostart configurations found.".to_string();
                            }
                            
                            match serde_json::from_str::<AutostartConfig>(&config_value) {
                                Ok(config) => {
                                    let mut output = "Autostart configurations:\n".to_string();
                                    
                                    for account in &config.twitch_irc {
                                        output.push_str(&format!("  twitch-irc - {} [ON]\n", account));
                                    }
                                    for account in &config.twitch_eventsub {
                                        output.push_str(&format!("  twitch-eventsub - {} [ON]\n", account));
                                    }
                                    for account in &config.discord {
                                        output.push_str(&format!("  discord - {} [ON]\n", account));
                                    }
                                    for account in &config.vrchat {
                                        output.push_str(&format!("  vrchat - {} [ON]\n", account));
                                    }
                                    
                                    output
                                }
                                Err(e) => format!("Error parsing autostart config: {}", e),
                            }
                        }
                        Err(e) => format!("Error getting autostart config: {}", e),
                    }
                }
                
                _ => "Usage: connection autostart <on|off|list> [platform] [account]".to_string(),
            }
        }
        
        "chat" => {
            if args.len() < 2 {
                return "Usage: connection chat <on|off> [platform] [account]".to_string();
            }
            
            match args[1] {
                "on" => {
                    let platform = args.get(2).map(|s| *s);
                    let account = args.get(3).map(|s| *s);
                    
                    match (platform, account) {
                        (Some(p), Some(a)) => {
                            tui_module.set_chat_state(true, Some(p.to_string()), Some(a.to_string())).await;
                            format!("Chat display enabled for platform={}, account={}", p, a)
                        }
                        (Some(p), None) => {
                            tui_module.set_chat_state(true, Some(p.to_string()), None).await;
                            format!("Chat display enabled for platform={}", p)
                        }
                        (None, None) => {
                            tui_module.set_chat_state(true, None, None).await;
                            "Chat display enabled for all platforms/accounts.".to_string()
                        }
                        _ => "Invalid chat filter combination.".to_string(),
                    }
                }
                
                "off" => {
                    tui_module.set_chat_state(false, None, None).await;
                    "Chat display disabled.".to_string()
                }
                
                _ => "Usage: connection chat <on|off> [platform] [account]".to_string(),
            }
        }
        
        "status" => {
            let request = maowbot_proto::maowbot::services::ListActiveRuntimesRequest {
                platforms: vec![],  // Empty means all platforms
            };
            
            let mut platform_client = client.platform.clone();
            match platform_client.list_active_runtimes(request).await {
                Ok(response) => {
                    let runtimes = response.into_inner().runtimes;
                    let mut output = "Connection Status:\n".to_string();
                    
                    if runtimes.is_empty() {
                        output.push_str("  No active connections.\n");
                    } else {
                        for runtime in runtimes {
                            output.push_str(&format!(
                                "  {} - {} [Running for {}s]\n",
                                runtime.platform,
                                runtime.account_name,
                                runtime.uptime_seconds
                            ));
                        }
                    }
                    
                    output
                }
                Err(e) => format!("Error getting connection status: {}", e),
            }
        }
        
        _ => format!("Unknown connection subcommand: {}", args[0]),
    }
}