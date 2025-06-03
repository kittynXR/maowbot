// Redeem command adapter for TUI
use maowbot_common_ui::{GrpcClient, commands::redeem::RedeemCommands};
use std::io::{stdin, stdout, Write};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct AutostartConfig {
    pub accounts: Vec<(String, String)>,
}

pub async fn handle_redeem_command(args: &[&str], client: &GrpcClient) -> String {
    if args.is_empty() {
        return "Usage: redeem <list|info|add|enable|pause|offline|setcost|setprompt|setplugin|setcommand|setinput|remove|sync>".to_string();
    }

    match args[0].to_lowercase().as_str() {
        "list" => {
            match RedeemCommands::list_redeems(client, Some("twitch-eventsub"), false, 100).await {
                Ok(result) => {
                    if result.data.redeems.is_empty() {
                        return "No redeems found for 'twitch-eventsub'.".to_string();
                    }

                    // Extract actual redeems from RedeemInfo
                    let redeems: Vec<_> = result.data.redeems.into_iter()
                        .filter_map(|info| info.redeem)
                        .collect();
                    
                    // Partition: web-app managed (is_managed=false) vs internally managed (is_managed=true)
                    let (web_app, internal): (Vec<_>, Vec<_>) =
                        redeems.into_iter().partition(|rd| rd.metadata.get("is_managed") != Some(&"true".to_string()));

                    // Try to get associated accounts from config service
                    let mut all_accounts: Vec<String> = Vec::new();
                    // TODO: Would need ConfigService integration to get autostart config
                    
                    let mut output = String::new();
                    
                    // Web-app managed
                    if !web_app.is_empty() {
                        output.push_str("--- Web-app managed redeems ---\n");
                        for rd in web_app {
                            let is_paused = rd.metadata.get("is_paused") == Some(&"true".to_string());
                            let status = if rd.is_active && !is_paused {
                                "ACTIVE"
                            } else if rd.is_active && is_paused {
                                "PAUSED"
                            } else {
                                "DISABLED"
                            };
                            
                            output.push_str(&format!(
                                " - {} [{}] cost={} prompt='{}' tid={}\n",
                                rd.reward_name,
                                status,
                                rd.cost,
                                if let Some(prompt) = rd.metadata.get("prompt") {
                                    if prompt.len() > 30 {
                                        format!("{}...", &prompt[..30])
                                    } else {
                                        prompt.clone()
                                    }
                                } else {
                                    String::new()
                                },
                                rd.metadata.get("twitch_id").cloned().unwrap_or_default()
                            ));
                        }
                        output.push('\n');
                    }
                    
                    // Internally managed
                    if !internal.is_empty() {
                        output.push_str("--- Internally managed redeems ---\n");
                        for rd in internal {
                            let is_paused = rd.metadata.get("is_paused") == Some(&"true".to_string());
                            let status = if rd.is_active && !is_paused {
                                "ACTIVE"
                            } else if rd.is_active && is_paused {
                                "PAUSED"
                            } else {
                                "DISABLED"
                            };
                            
                            output.push_str(&format!(
                                " - {} [{}] cost={} plugin={} cmd={}\n",
                                rd.reward_name,
                                status,
                                rd.cost,
                                rd.metadata.get("plugin_id").map(|s| s.as_str()).unwrap_or("BUILTIN"),
                                rd.metadata.get("command_name").map(|s| s.as_str()).unwrap_or("(none)")
                            ));
                        }
                    }
                    
                    output
                }
                Err(e) => format!("Error listing redeems: {}", e),
            }
        }
        
        "info" => {
            if args.len() < 2 {
                return "Usage: redeem info <redeemName>".to_string();
            }
            let redeem_name = args[1];
            
            match RedeemCommands::find_redeem_by_name(client, "twitch-eventsub", redeem_name).await {
                Ok(Some(rd)) => {
                    let mut out = format!("Redeem: {}\n", rd.reward_name);
                    out.push_str(&format!("ID: {}\n", rd.redeem_id));
                    out.push_str(&format!("Twitch ID: {}\n", rd.metadata.get("twitch_id").cloned().unwrap_or_default()));
                    out.push_str(&format!("Cost: {}\n", rd.cost));
                    out.push_str(&format!("Enabled: {}\n", rd.is_active));
                    out.push_str(&format!("Paused: {}\n", rd.metadata.get("is_paused") == Some(&"true".to_string())));
                    let is_managed = rd.metadata.get("is_managed") == Some(&"true".to_string());
                    out.push_str(&format!("Managed: {} ({})\n", 
                        is_managed,
                        if is_managed { "internally" } else { "web-app" }
                    ));
                    out.push_str(&format!("Skip Queue: {}\n", rd.metadata.get("should_skip_request_queue") == Some(&"true".to_string())));
                    out.push_str(&format!("Input Required: {}\n", rd.metadata.get("input_required") == Some(&"true".to_string())));
                    out.push_str(&format!("Plugin: {}\n", 
                        rd.metadata.get("plugin_id").map(|s| s.as_str()).unwrap_or("BUILTIN")
                    ));
                    out.push_str(&format!("Command: {}\n", 
                        rd.metadata.get("command_name").map(|s| s.as_str()).unwrap_or("(none)")
                    ));
                    out.push_str(&format!("Prompt: {}\n", rd.metadata.get("prompt").cloned().unwrap_or_default()));
                    out
                }
                Ok(None) => format!("Redeem '{}' not found.", redeem_name),
                Err(e) => format!("Error: {}", e),
            }
        }
        
        "add" => {
            if args.len() < 3 {
                return "Usage: redeem add <redeemName> <cost>".to_string();
            }
            let redeem_name = args[1];
            let cost = match args[2].parse::<i32>() {
                Ok(c) if c >= 0 => c,
                _ => return "Cost must be a non-negative integer.".to_string(),
            };
            
            println!("Enter prompt text (or leave blank): ");
            print!("> ");
            let _ = stdout().flush();
            let mut prompt = String::new();
            let _ = stdin().read_line(&mut prompt);
            let prompt = prompt.trim();
            let prompt_opt = if prompt.is_empty() { None } else { Some(prompt) };
            
            match RedeemCommands::create_redeem(
                client,
                "twitch-eventsub",
                redeem_name,
                None, // twitch_id will be assigned by API
                None, // plugin_id
                cost,
                true, // is_enabled
                false, // is_paused
                false, // should_skip_request_queue
                true, // is_managed (internally)
                prompt_opt,
                false, // input_required
                None, // command_name
            ).await {
                Ok(result) => format!(
                    "Created redeem '{}' with cost {} (id={}).",
                    result.data.redeem.reward_name,
                    cost,
                    result.data.redeem.redeem_id
                ),
                Err(e) => format!("Error creating redeem: {}", e),
            }
        }
        
        "enable" => {
            if args.len() < 2 {
                return "Usage: redeem enable <redeemName>".to_string();
            }
            let redeem_name = args[1];
            
            match RedeemCommands::set_redeem_state(client, "twitch-eventsub", redeem_name, Some(true), Some(false)).await {
                Ok(_) => format!("Enabled redeem '{}'.", redeem_name),
                Err(e) => format!("Error enabling redeem: {}", e),
            }
        }
        
        "pause" => {
            if args.len() < 2 {
                return "Usage: redeem pause <redeemName>".to_string();
            }
            let redeem_name = args[1];
            
            match RedeemCommands::set_redeem_state(client, "twitch-eventsub", redeem_name, Some(true), Some(true)).await {
                Ok(_) => format!("Paused redeem '{}'.", redeem_name),
                Err(e) => format!("Error pausing redeem: {}", e),
            }
        }
        
        "offline" | "disable" => {
            if args.len() < 2 {
                return "Usage: redeem offline <redeemName>".to_string();
            }
            let redeem_name = args[1];
            
            match RedeemCommands::set_redeem_state(client, "twitch-eventsub", redeem_name, Some(false), None).await {
                Ok(_) => format!("Disabled redeem '{}'.", redeem_name),
                Err(e) => format!("Error disabling redeem: {}", e),
            }
        }
        
        "setcost" => {
            if args.len() < 3 {
                return "Usage: redeem setcost <redeemName> <newCost>".to_string();
            }
            let redeem_name = args[1];
            let cost = match args[2].parse::<i32>() {
                Ok(c) if c >= 0 => c,
                _ => return "Cost must be a non-negative integer.".to_string(),
            };
            
            match RedeemCommands::set_redeem_cost(client, "twitch-eventsub", redeem_name, cost).await {
                Ok(_) => format!("Updated cost for '{}' to {}.", redeem_name, cost),
                Err(e) => format!("Error updating cost: {}", e),
            }
        }
        
        "setprompt" => {
            if args.len() < 3 {
                return "Usage: redeem setprompt <redeemName> <newPrompt>".to_string();
            }
            let redeem_name = args[1];
            let prompt = args[2..].join(" ");
            
            match RedeemCommands::set_redeem_prompt(client, "twitch-eventsub", redeem_name, &prompt).await {
                Ok(_) => format!("Updated prompt for '{}'.", redeem_name),
                Err(e) => format!("Error updating prompt: {}", e),
            }
        }
        
        "setplugin" => {
            if args.len() < 3 {
                return "Usage: redeem setplugin <redeemName> <pluginId|builtin>".to_string();
            }
            let redeem_name = args[1];
            let plugin_id = if args[2].eq_ignore_ascii_case("builtin") {
                ""
            } else {
                args[2]
            };
            
            match RedeemCommands::update_plugin(client, "twitch-eventsub", redeem_name, plugin_id).await {
                Ok(_) => format!(
                    "Updated plugin for '{}' to {}.",
                    redeem_name,
                    if plugin_id.is_empty() { "BUILTIN" } else { plugin_id }
                ),
                Err(e) => format!("Error updating plugin: {}", e),
            }
        }
        
        "setcommand" => {
            if args.len() < 3 {
                return "Usage: redeem setcommand <redeemName> <commandName|none>".to_string();
            }
            let redeem_name = args[1];
            let command_name = if args[2].eq_ignore_ascii_case("none") {
                ""
            } else {
                args[2]
            };
            
            match RedeemCommands::update_command(client, "twitch-eventsub", redeem_name, command_name).await {
                Ok(_) => format!(
                    "Updated command for '{}' to {}.",
                    redeem_name,
                    if command_name.is_empty() { "(none)" } else { command_name }
                ),
                Err(e) => format!("Error updating command: {}", e),
            }
        }
        
        "setinput" => {
            if args.len() < 3 {
                return "Usage: redeem setinput <redeemName> <true|false>".to_string();
            }
            let redeem_name = args[1];
            let input_required = match args[2].to_lowercase().as_str() {
                "true" | "yes" | "1" => true,
                "false" | "no" | "0" => false,
                _ => return "Input required must be 'true' or 'false'.".to_string(),
            };
            
            match RedeemCommands::update_input_required(client, "twitch-eventsub", redeem_name, input_required).await {
                Ok(_) => format!("Updated input_required for '{}' to {}.", redeem_name, input_required),
                Err(e) => format!("Error updating input_required: {}", e),
            }
        }
        
        "remove" => {
            if args.len() < 2 {
                return "Usage: redeem remove <redeemName>".to_string();
            }
            let redeem_name = args[1];
            
            match RedeemCommands::find_redeem_by_name(client, "twitch-eventsub", redeem_name).await {
                Ok(Some(rd)) => {
                    if rd.metadata.get("is_managed") != Some(&"true".to_string()) {
                        return format!("Cannot remove web-app managed redeem '{}'. Use Twitch dashboard.", redeem_name);
                    }
                    
                    println!("Are you sure you want to remove redeem '{}'? (y/n)", redeem_name);
                    print!("> ");
                    let _ = stdout().flush();
                    let mut confirm = String::new();
                    let _ = stdin().read_line(&mut confirm);
                    
                    if confirm.trim().eq_ignore_ascii_case("y") {
                        match RedeemCommands::delete_redeem(client, &rd.redeem_id).await {
                            Ok(_) => format!("Removed redeem '{}'.", redeem_name),
                            Err(e) => format!("Error removing redeem: {}", e),
                        }
                    } else {
                        "Removal cancelled.".to_string()
                    }
                }
                Ok(None) => format!("Redeem '{}' not found.", redeem_name),
                Err(e) => format!("Error: {}", e),
            }
        }
        
        "sync" => {
            if args.len() < 2 {
                return "Usage: redeem sync <accountName>".to_string();
            }
            let account_name = args[1];
            
            match RedeemCommands::sync_redeems(client, "twitch-eventsub").await {
                Ok(result) => format!(
                    "Sync complete for '{}': {} added, {} updated, {} removed.",
                    account_name,
                    result.data.added_count,
                    result.data.updated_count,
                    result.data.removed_count
                ),
                Err(e) => format!("Error syncing redeems: {}", e),
            }
        }
        
        _ => "Usage: redeem <list|info|add|enable|pause|offline|setcost|setprompt|setplugin|setcommand|setinput|remove|sync>".to_string(),
    }
}