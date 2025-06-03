// Account command adapter for TUI
use maowbot_common_ui::{GrpcClient, commands::account::AccountCommands};
use std::io::{Write, stdin, stdout};

pub async fn handle_account_command(args: &[&str], client: &GrpcClient) -> String {
    if args.is_empty() {
        return "Usage: account <add|remove|list|show|refresh|type> [platform] [usernameOrUUID]".to_string();
    }

    match args[0] {
        "add" => {
            if args.len() < 3 {
                return "Usage: account add <platform> <typed_global_username>".to_string();
            }
            let platform = args[1];
            let typed_name = args[2];
            
            // Ask if broadcaster account
            println!("Is this your broadcaster account [Y/n]");
            print!("> ");
            let _ = stdout().flush();
            let mut line = String::new();
            let _ = stdin().read_line(&mut line);
            let trimmed = line.trim().to_lowercase();
            let is_broadcaster = trimmed.is_empty() || trimmed == "y" || trimmed == "yes";
            
            // Ask if teammate account
            let mut is_teammate = false;
            if !is_broadcaster {
                println!("Is this a teammate account [y/N]");
                print!("> ");
                let _ = stdout().flush();
                let mut line2 = String::new();
                let _ = stdin().read_line(&mut line2);
                let trimmed2 = line2.trim().to_lowercase();
                is_teammate = trimmed2 == "y" || trimmed2 == "yes";
            }
            
            // Ask if bot account
            let is_bot = !is_broadcaster && !is_teammate;
            if is_bot {
                println!("Is this a bot account [Y/n]");
                print!("> ");
                let _ = stdout().flush();
                let mut line3 = String::new();
                let _ = stdin().read_line(&mut line3);
                let trimmed3 = line3.trim().to_lowercase();
                if !(trimmed3.is_empty() || trimmed3 == "y" || trimmed3 == "yes") {
                    return "Account must be either broadcaster, teammate, or bot".to_string();
                }
            }
            
            match AccountCommands::add_account(client, platform, typed_name, is_bot, is_broadcaster, is_teammate).await {
                Ok(result) => {
                    if let Some(auth_url) = result.auth_url {
                        println!("Open this URL to authenticate:\n  {}", auth_url);
                        if is_bot {
                            println!("(Bot account) Attempting incognito. If it fails, open manually.\nOr sign out of your main account first.\n");
                        }
                        // The actual OAuth flow would need to be handled here
                        // For now, just return a message
                        "OAuth flow initiated. Please complete authentication in your browser.".to_string()
                    } else {
                        result.message
                    }
                }
                Err(e) => format!("Error adding account: {}", e),
            }
        }
        
        "remove" => {
            if args.len() < 3 {
                return "Usage: account remove <platform> <usernameOrUUID>".to_string();
            }
            let platform = args[1];
            let user_str = args[2];
            
            match AccountCommands::remove_account(client, platform, user_str).await {
                Ok(result) => result.message,
                Err(e) => format!("Error removing account: {}", e),
            }
        }
        
        "list" => {
            let platform = args.get(1).map(|s| *s);
            
            match AccountCommands::list_accounts(client, platform).await {
                Ok(result) => {
                    if result.credentials.is_empty() {
                        "No stored platform credentials.\n".to_string()
                    } else {
                        let mut out = String::new();
                        out.push_str("Stored platform credentials:\n");
                        for cred in result.credentials {
                            out.push_str(&format!(
                                " - user='{}' platform={} is_bot={} credential_id={}\n",
                                cred.username,
                                cred.platform,
                                cred.is_bot,
                                cred.credential_id
                            ));
                        }
                        out
                    }
                }
                Err(e) => format!("Error listing accounts: {}", e),
            }
        }
        
        "show" => {
            if args.len() < 3 {
                return "Usage: account show <platform> <usernameOrUUID>".to_string();
            }
            let platform = args[1];
            let user_str = args[2];
            
            match AccountCommands::show_account(client, platform, user_str).await {
                Ok(result) => {
                    if let Some(cred) = result.credential {
                        let mut out = String::new();
                        out.push_str(&format!("platform={}\n", cred.platform));
                        out.push_str(&format!("user_id={}\n", cred.user_id));
                        out.push_str(&format!("is_bot={}\n", cred.is_bot));
                        out.push_str(&format!("is_active={}\n", cred.is_active));
                        out.push_str(&format!("expires_at={:?}\n", cred.expires_at));
                        out.push_str(&format!("created_at={}\n", cred.created_at));
                        out.push_str(&format!("last_refreshed={:?}\n", cred.last_refreshed));
                        out
                    } else {
                        format!("No credentials found for platform={}, user='{}'", platform, user_str)
                    }
                }
                Err(e) => format!("Error showing account: {}", e),
            }
        }
        
        "refresh" => {
            if args.len() < 3 {
                return "Usage: account refresh <platform> <usernameOrUUID>".to_string();
            }
            let platform = args[1];
            let user_str = args[2];
            
            match AccountCommands::refresh_account(client, platform, user_str).await {
                Ok(result) => result.message,
                Err(e) => format!("Error refreshing account: {}", e),
            }
        }
        
        "type" => {
            if args.len() < 4 {
                return "Usage: account type <platform> <usernameOrUUID> <bot|broadcaster|teammate>".to_string();
            }
            let platform = args[1];
            let user_str = args[2];
            let role_flag = args[3];
            
            let (is_bot, is_broadcaster, is_teammate) = match role_flag.to_lowercase().as_str() {
                "bot" => (true, false, false),
                "broadcaster" => (false, true, false),
                "teammate" => (false, false, true),
                _ => return format!("Unrecognized role flag: '{}'; must be one of: bot, broadcaster, teammate", role_flag),
            };
            
            match AccountCommands::set_account_type(client, platform, user_str, is_bot, is_broadcaster, is_teammate).await {
                Ok(result) => result.message,
                Err(e) => format!("Error setting account type: {}", e),
            }
        }
        
        _ => "Usage: account <add|remove|list|show|refresh|type> [platform] [usernameOrUUID]".to_string(),
    }
}