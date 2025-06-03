// Command command adapter for TUI
use maowbot_common_ui::{GrpcClient, commands::command::CommandCommands};
use std::io::{stdin, stdout, Write};
use uuid::Uuid;

pub async fn handle_command_command(args: &[&str], client: &GrpcClient) -> String {
    if args.is_empty() {
        return "Usage: command <list|setcooldown|setwarnonce|setrespond|setplatform|enable|disable> [args...]".to_string();
    }
    
    match args[0].to_lowercase().as_str() {
        "list" => {
            // If no platform specified, list from all known platforms
            if args.len() == 1 {
                let known_platforms = vec!["twitch-irc", "twitch", "vrchat", "discord", "twitch-eventsub"];
                let mut out = String::new();
                
                for plat in &known_platforms {
                    match CommandCommands::list_commands(client, Some(plat), false, 100).await {
                        Ok(result) if !result.data.commands.is_empty() => {
                            out.push_str(&format!("Commands for platform '{}':\n", plat));
                            for info in result.data.commands {
                                if let Some(c) = info.command {
                                    let warnonce = c.metadata.get("cooldown_warnonce").map(|v| v == "true").unwrap_or(false);
                                    let respond = c.metadata.get("respond_with_credential");
                                    out.push_str(&format!(
                                        " - {} (id={}) active={} cd={}s warnonce={} respond={:?}\n",
                                        c.name,
                                        c.command_id,
                                        c.is_active,
                                        c.cooldown_seconds,
                                        warnonce,
                                        respond
                                    ));
                                }
                            }
                            out.push('\n');
                        }
                        _ => { /* skip if empty or error */ }
                    }
                }
                
                if out.is_empty() {
                    "No commands found on any platform.".to_string()
                } else {
                    out
                }
            } else {
                // Platform specified
                let platform = args[1];
                match CommandCommands::list_commands(client, Some(platform), false, 100).await {
                    Ok(result) => {
                        if result.data.commands.is_empty() {
                            format!("No commands found for platform '{}'.", platform)
                        } else {
                            let mut out = format!("Commands for platform '{}':\n", platform);
                            for info in result.data.commands {
                                if let Some(c) = info.command {
                                    let warnonce = c.metadata.get("cooldown_warnonce").map(|v| v == "true").unwrap_or(false);
                                    let respond = c.metadata.get("respond_with_credential");
                                    out.push_str(&format!(
                                        " - {} (id={}) active={} cd={}s warnonce={} respond={:?}\n",
                                        c.name,
                                        c.command_id,
                                        c.is_active,
                                        c.cooldown_seconds,
                                        warnonce,
                                        respond
                                    ));
                                }
                            }
                            out
                        }
                    }
                    Err(e) => format!("Error listing commands: {}", e),
                }
            }
        }
        
        "setcooldown" => {
            if args.len() < 3 {
                return "Usage: command setcooldown <commandName> <seconds> [platform]".to_string();
            }
            let cmd_name = args[1];
            let seconds = match args[2].parse::<i32>() {
                Ok(s) if s >= 0 => s,
                _ => return "Cooldown seconds must be a non-negative integer.".to_string(),
            };
            let platform = args.get(3).copied().unwrap_or("twitch-irc");
            
            match CommandCommands::update_cooldown(client, platform, cmd_name, seconds).await {
                Ok(result) => format!(
                    "Updated cooldown for '{}' on platform '{}' to {} seconds.",
                    result.data.command.name,
                    platform,
                    seconds
                ),
                Err(e) => format!("Error updating cooldown: {}", e),
            }
        }
        
        "setwarnonce" => {
            if args.len() < 3 {
                return "Usage: command setwarnonce <commandName> <true|false> [platform]".to_string();
            }
            let cmd_name = args[1];
            let warnonce = match args[2].to_lowercase().as_str() {
                "true" | "yes" | "1" => true,
                "false" | "no" | "0" => false,
                _ => return "Warnonce must be 'true' or 'false'.".to_string(),
            };
            let platform = args.get(3).copied().unwrap_or("twitch-irc");
            
            match CommandCommands::update_warnonce(client, platform, cmd_name, warnonce).await {
                Ok(result) => format!(
                    "Updated warnonce for '{}' on platform '{}' to {}.",
                    result.data.command.name,
                    platform,
                    warnonce
                ),
                Err(e) => format!("Error updating warnonce: {}", e),
            }
        }
        
        "setrespond" => {
            if args.len() < 3 {
                return "Usage: command setrespond <commandName> <credentialId|accountName|none> [platform]".to_string();
            }
            let cmd_name = args[1];
            let respond_arg = args[2];
            let platform = args.get(3).copied().unwrap_or("twitch-irc");
            
            // Determine the credential_id
            let credential_id = if respond_arg.eq_ignore_ascii_case("none") {
                None
            } else if Uuid::parse_str(respond_arg).is_ok() {
                Some(respond_arg.to_string())
            } else {
                // Try to find credential by account name
                // This would require CredentialService integration
                // For now, we'll just use the provided value
                Some(respond_arg.to_string())
            };
            
            match CommandCommands::update_respond_with(client, platform, cmd_name, credential_id.clone()).await {
                Ok(result) => {
                    if credential_id.is_none() {
                        format!(
                            "Cleared respond_with_credential for '{}' on platform '{}'.",
                            result.data.command.name,
                            platform
                        )
                    } else {
                        format!(
                            "Updated respond_with_credential for '{}' on platform '{}' to '{}'.",
                            result.data.command.name,
                            platform,
                            credential_id.unwrap()
                        )
                    }
                }
                Err(e) => format!("Error updating respond_with_credential: {}", e),
            }
        }
        
        "setplatform" => {
            if args.len() < 3 {
                return "Usage: command setplatform <commandName> <newPlatform> [oldPlatform]".to_string();
            }
            let cmd_name = args[1];
            let new_platform = args[2];
            let old_platform = args.get(3).copied().unwrap_or("twitch-irc");
            
            // Find the command on old platform
            match CommandCommands::find_command_by_name(client, old_platform, cmd_name).await {
                Ok(Some(mut cmd)) => {
                    // Delete from old platform
                    if let Err(e) = CommandCommands::delete_command(client, &cmd.command_id).await {
                        return format!("Error deleting command from old platform: {}", e);
                    }
                    
                    // Create on new platform
                    let plugin_id = cmd.metadata.get("plugin_id").map(|s| s.as_str());
                    let warnonce = cmd.metadata.get("cooldown_warnonce").map(|v| v == "true").unwrap_or(false);
                    let respond_cred = cmd.metadata.get("respond_with_credential").map(|s| s.as_str());
                    
                    match CommandCommands::create_command(
                        client,
                        new_platform,
                        &cmd.name,
                        plugin_id,
                        cmd.is_active,
                        cmd.cooldown_seconds,
                        warnonce,
                        respond_cred,
                    ).await {
                        Ok(_) => format!(
                            "Moved command '{}' from platform '{}' to '{}'.",
                            cmd_name,
                            old_platform,
                            new_platform
                        ),
                        Err(e) => format!("Error creating command on new platform: {}", e),
                    }
                }
                Ok(None) => format!("Command '{}' not found on platform '{}'.", cmd_name, old_platform),
                Err(e) => format!("Error finding command: {}", e),
            }
        }
        
        "enable" => {
            if args.len() < 2 {
                return "Usage: command enable <commandName> [platform]".to_string();
            }
            let cmd_name = args[1];
            let platform = args.get(2).copied().unwrap_or("twitch-irc");
            
            match CommandCommands::set_active(client, platform, cmd_name, true).await {
                Ok(result) => format!(
                    "Enabled command '{}' on platform '{}'.",
                    result.data.command.name,
                    platform
                ),
                Err(e) => format!("Error enabling command: {}", e),
            }
        }
        
        "disable" => {
            if args.len() < 2 {
                return "Usage: command disable <commandName> [platform]".to_string();
            }
            let cmd_name = args[1];
            let platform = args.get(2).copied().unwrap_or("twitch-irc");
            
            match CommandCommands::set_active(client, platform, cmd_name, false).await {
                Ok(result) => format!(
                    "Disabled command '{}' on platform '{}'.",
                    result.data.command.name,
                    platform
                ),
                Err(e) => format!("Error disabling command: {}", e),
            }
        }
        
        "create" => {
            if args.len() < 3 {
                return "Usage: command create <commandName> <platform>".to_string();
            }
            let cmd_name = args[1];
            let platform = args[2];
            
            println!("Creating new command '{}' on platform '{}'", cmd_name, platform);
            println!("Enter plugin_id (or leave blank for built-in): ");
            print!("> ");
            let _ = stdout().flush();
            let mut plugin_id = String::new();
            let _ = stdin().read_line(&mut plugin_id);
            let plugin_id = plugin_id.trim();
            let plugin_opt = if plugin_id.is_empty() { None } else { Some(plugin_id) };
            
            match CommandCommands::create_command(
                client,
                platform,
                cmd_name,
                plugin_opt,
                true,  // is_active
                0,     // cooldown_seconds
                false, // cooldown_warnonce
                None,  // respond_with_credential
            ).await {
                Ok(result) => format!(
                    "Created command '{}' on platform '{}' (id={}).",
                    result.data.command.name,
                    platform,
                    result.data.command.command_id
                ),
                Err(e) => format!("Error creating command: {}", e),
            }
        }
        
        "delete" => {
            if args.len() < 2 {
                return "Usage: command delete <commandName> [platform]".to_string();
            }
            let cmd_name = args[1];
            let platform = args.get(2).copied().unwrap_or("twitch-irc");
            
            match CommandCommands::find_command_by_name(client, platform, cmd_name).await {
                Ok(Some(cmd)) => {
                    println!("Are you sure you want to delete command '{}' on platform '{}'? (y/n)", cmd_name, platform);
                    print!("> ");
                    let _ = stdout().flush();
                    let mut confirm = String::new();
                    let _ = stdin().read_line(&mut confirm);
                    
                    if confirm.trim().eq_ignore_ascii_case("y") {
                        match CommandCommands::delete_command(client, &cmd.command_id).await {
                            Ok(_) => format!("Deleted command '{}' from platform '{}'.", cmd_name, platform),
                            Err(e) => format!("Error deleting command: {}", e),
                        }
                    } else {
                        "Deletion cancelled.".to_string()
                    }
                }
                Ok(None) => format!("Command '{}' not found on platform '{}'.", cmd_name, platform),
                Err(e) => format!("Error finding command: {}", e),
            }
        }
        
        _ => "Usage: command <list|setcooldown|setwarnonce|setrespond|setplatform|enable|disable|create|delete> [args...]".to_string(),
    }
}