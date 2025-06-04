// Connection command adapter for TUI - consolidates start/stop/autostart/chat
use maowbot_common_ui::{GrpcClient, commands::connectivity::{ConnectivityCommands, AutostartConfig}};
use crate::tui_module_simple::SimpleTuiModule;
use std::sync::Arc;
use maowbot_proto::maowbot::services::{
    GetConfigRequest, ListActiveRuntimesRequest, ListCredentialsRequest,
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
                // List available accounts and prompt for selection
                match ConnectivityCommands::list_platform_accounts(client, platform).await {
                    Ok(accounts) => {
                        if accounts.is_empty() {
                            return format!("No accounts found for platform {}", platform);
                        }
                        
                        // Show available accounts
                        println!("Available accounts for {}:", platform);
                        for (idx, account) in accounts.iter().enumerate() {
                            println!("  {}. {} ({})", idx + 1, account.user_name, account.display_name);
                        }
                        println!("\nEnter account number (1-{}) or press Enter to start all:", accounts.len());
                        
                        // Read user input
                        use std::io::{self, Write};
                        print!("> ");
                        let _ = io::stdout().flush();
                        let mut input = String::new();
                        if let Err(e) = io::stdin().read_line(&mut input) {
                            return format!("Error reading input: {}", e);
                        }
                        
                        let trimmed = input.trim();
                        if trimmed.is_empty() {
                            // Start all accounts
                            let mut results = Vec::new();
                            for account in &accounts {
                                match ConnectivityCommands::start_platform(client, platform, &account.user_name).await {
                                    Ok(result) => results.push(format!("✓ Started {} for {}", 
                                        result.platform, result.account)),
                                    Err(e) => results.push(format!("✗ Failed to start {} for {}: {}", 
                                        platform, account.user_name, e)),
                                }
                            }
                            return results.join("\n");
                        } else if let Ok(num) = trimmed.parse::<usize>() {
                            if num > 0 && num <= accounts.len() {
                                let selected = &accounts[num - 1];
                                match ConnectivityCommands::start_platform(client, platform, &selected.user_name).await {
                                    Ok(result) => return format!("Started {} runtime for account {}", 
                                        result.platform, result.account),
                                    Err(e) => return format!("Error starting platform: {}", e),
                                }
                            } else {
                                return format!("Invalid selection. Please enter a number between 1 and {}.", accounts.len());
                            }
                        } else {
                            return "Invalid input. Please enter a number or press Enter.".to_string();
                        }
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
            // Get active runtimes
            let runtime_request = ListActiveRuntimesRequest {
                platforms: vec![],  // Empty means all platforms
            };
            
            let mut platform_client = client.platform.clone();
            let active_runtimes = match platform_client.list_active_runtimes(runtime_request).await {
                Ok(response) => response.into_inner().runtimes,
                Err(e) => return format!("Error getting connection status: {}", e),
            };
            
            // Get all credentials
            let cred_request = ListCredentialsRequest {
                platforms: vec![],  // Empty means all platforms
                include_expired: false,
                active_only: true,
                page: None,
            };
            
            let mut cred_client = client.credential.clone();
            let all_credentials = match cred_client.list_credentials(cred_request).await {
                Ok(response) => response.into_inner().credentials,
                Err(e) => return format!("Error getting credentials: {}", e),
            };
            
            let mut output = String::new();
            output.push_str("Connection Status:\n\n");
            
            // Group by platform
            let platforms = ["TwitchIrc", "TwitchEventSub", "Discord", "VRChat"];
            
            for platform_name in &platforms {
                output.push_str(&format!("{}:\n", platform_name));
                
                // Find all credentials for this platform
                let platform_creds: Vec<_> = all_credentials.iter()
                    .filter(|cred_info| {
                        if let Some(cred) = &cred_info.credential {
                            if let Ok(plat) = maowbot_proto::maowbot::common::Platform::try_from(cred.platform) {
                                let display_name = match plat {
                                    maowbot_proto::maowbot::common::Platform::TwitchIrc => "TwitchIrc",
                                    maowbot_proto::maowbot::common::Platform::TwitchEventsub => "TwitchEventSub",
                                    maowbot_proto::maowbot::common::Platform::Discord => "Discord",
                                    maowbot_proto::maowbot::common::Platform::Vrchat => "VRChat",
                                    _ => return false,
                                };
                                display_name == *platform_name
                            } else {
                                false
                            }
                        } else {
                            false
                        }
                    })
                    .collect();
                
                if platform_creds.is_empty() {
                    output.push_str("  (no credentials)\n");
                } else {
                    for cred_info in platform_creds {
                        if let Some(cred) = &cred_info.credential {
                            let username = &cred.user_name;
                            let display_name = if let Some(user) = &cred_info.user {
                                &user.global_username
                            } else {
                                username
                            };
                            
                            // Check if this account is running
                            // Runtime platforms come as "twitch-irc", "twitch-eventsub", etc.
                            let platform_runtime_name = match *platform_name {
                                "TwitchIrc" => "twitch-irc",
                                "TwitchEventSub" => "twitch-eventsub", 
                                "Discord" => "discord",
                                "VRChat" => "vrchat",
                                _ => platform_name,
                            };
                            
                            let is_running = active_runtimes.iter().any(|rt| {
                                rt.platform == platform_runtime_name && 
                                rt.account_name == *username
                            });
                            
                            if is_running {
                                let runtime = active_runtimes.iter()
                                    .find(|rt| rt.platform == platform_runtime_name && 
                                               rt.account_name == *username)
                                    .unwrap();
                                output.push_str(&format!(
                                    "  - {} ({}) [CONNECTED - {}s]\n",
                                    username, display_name, runtime.uptime_seconds
                                ));
                            } else {
                                output.push_str(&format!(
                                    "  - {} ({}) [Available]\n",
                                    username, display_name
                                ));
                            }
                        }
                    }
                }
                output.push('\n');
            }
            
            output
        }
        
        _ => format!("Unknown connection subcommand: {}", args[0]),
    }
}