// Connection command adapter for TUI - consolidates start/stop/autostart/chat
use maowbot_common_ui::{GrpcClient, commands::connectivity::ConnectivityCommands};
use crate::tui_module_simple::SimpleTuiModule;
use std::sync::Arc;
use maowbot_proto::maowbot::services::{
    ListActiveRuntimesRequest, ListCredentialsRequest,
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
            
            // Handle OBS instances specially
            if platform == "obs" {
                if account.is_empty() {
                    return "Usage: connection start obs <instance_number>".to_string();
                }
                // Convert instance number to obs-N format
                let instance_account = if account.starts_with("obs-") {
                    account.to_string()
                } else {
                    format!("obs-{}", account)
                };
                
                match ConnectivityCommands::start_platform(client, platform, &instance_account).await {
                    Ok(result) => return format!("Started OBS instance {}", account),
                    Err(e) => return format!("Error starting OBS instance: {}", e),
                }
            }
            
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
                            println!("  {}. {} ({})", idx + 1, account.display_name, account.display_name);
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
                                // Pass display_name which contains the global_username
                                match ConnectivityCommands::start_platform(client, platform, &account.display_name).await {
                                    Ok(result) => results.push(format!("✓ Started {} for {}", 
                                        result.platform, account.user_name)),
                                    Err(e) => results.push(format!("✗ Failed to start {} for {}: {}", 
                                        platform, account.user_name, e)),
                                }
                            }
                            return results.join("\n");
                        } else if let Ok(num) = trimmed.parse::<usize>() {
                            if num > 0 && num <= accounts.len() {
                                let selected = &accounts[num - 1];
                                // Pass display_name which contains the global_username
                                match ConnectivityCommands::start_platform(client, platform, &selected.display_name).await {
                                    Ok(result) => return format!("Started {} runtime for account {}", 
                                        result.platform, selected.user_name),
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
            
            // Pass the account name directly
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
            
            if account.is_empty() {
                return "Please specify an account to stop.".to_string();
            }
            
            // Handle OBS instances specially
            if platform == "obs" {
                // Convert instance number to obs-N format
                let instance_account = if account.starts_with("obs-") {
                    account.to_string()
                } else {
                    format!("obs-{}", account)
                };
                
                match ConnectivityCommands::stop_platform(client, platform, &instance_account).await {
                    Ok(result) => return format!("Stopped OBS instance {}", account),
                    Err(e) => return format!("Error stopping OBS instance: {}", e),
                }
            }
            
            // Pass the account name directly
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
                    match ConnectivityCommands::list_autostart_entries(client).await {
                        Ok(entries) => {
                            if entries.is_empty() {
                                "No autostart configurations found.".to_string()
                            } else {
                                let mut output = "Autostart configurations:\n".to_string();
                                for (platform, account, enabled) in entries {
                                    output.push_str(&format!("  {} - {} [{}]\n", 
                                        platform, account,
                                        if enabled { "ON" } else { "OFF" }
                                    ));
                                }
                                output
                            }
                        }
                        Err(e) => format!("Error listing autostart configurations: {}", e),
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
            
            // Create a mapping of user_id to username from active runtimes
            // Since server returns user_id as account_name in runtimes
            let mut user_id_to_username = std::collections::HashMap::new();
            for cred_info in &all_credentials {
                if let Some(cred) = &cred_info.credential {
                    user_id_to_username.insert(cred.user_id.clone(), cred.user_name.clone());
                }
            }
            
            // Group by platform
            let platforms = ["TwitchIrc", "TwitchEventSub", "Discord", "VRChat", "OBS"];
            
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
                                    maowbot_proto::maowbot::common::Platform::Obs => "OBS",
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
                                "OBS" => "obs",
                                _ => platform_name,
                            };
                            
                            // Special handling for OBS instances
                            if *platform_name == "OBS" {
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
                                        "  - Instance {} [CONNECTED - {}s]\n",
                                        username.replace("obs-", ""), runtime.uptime_seconds
                                    ));
                                } else {
                                    output.push_str(&format!(
                                        "  - Instance {} [Available]\n",
                                        username.replace("obs-", "")
                                    ));
                                }
                            } else {
                                // Check if runtime is active for this account
                                // Server might return either username or user_id as account_name
                                let user_id = &cred.user_id;
                                let is_running = active_runtimes.iter().any(|rt| {
                                    rt.platform == platform_runtime_name && 
                                    (rt.account_name == *username || rt.account_name == *user_id)
                                });
                                
                                if is_running {
                                    let runtime = active_runtimes.iter()
                                        .find(|rt| rt.platform == platform_runtime_name && 
                                                   (rt.account_name == *username || rt.account_name == *user_id))
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
                }
                output.push('\n');
            }
            
            output
        }
        
        _ => format!("Unknown connection subcommand: {}", args[0]),
    }
}