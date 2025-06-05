// Redeem command adapter for TUI
use maowbot_common_ui::{GrpcClient, commands::redeem::RedeemCommands};
use maowbot_proto::maowbot::common::Redeem;
use std::io::{stdin, stdout, Write};
use uuid::Uuid;

pub async fn handle_redeem_command(args: &[&str], client: &GrpcClient) -> String {
    if args.is_empty() {
        return "Usage: redeem <list|info|add|enable|disable|pause|unpause|setcost|setprompt|setplugin|setcommand|setinput|remove|sync>".to_string();
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

                    let mut output = String::from("Current Redeems (twitch-eventsub):\n\n");
                    
                    // Web-app managed
                    if web_app.is_empty() {
                        output.push_str("[Web-app managed redeems]\n(no items)\n\n");
                    } else {
                        output.push_str("[Web-app managed redeems]\n");
                        output.push_str(&format_table(&web_app, false));
                        output.push_str("\n");
                    }
                    
                    // Internally managed
                    if internal.is_empty() {
                        output.push_str("[Internally managed redeems]\n(no items)\n\n");
                    } else {
                        output.push_str("[Internally managed redeems]\n");
                        output.push_str(&format_table(&internal, true));
                        output.push_str("\n");
                    }
                    
                    output
                }
                Err(e) => format!("Error listing redeems: {}", e),
            }
        }
        
        "info" => {
            if args.len() < 2 {
                return "Usage: redeem info <redeemNameOrUuidOrNumber>".to_string();
            }
            
            // Join all args after "info" to handle multi-word names
            let user_input = args[1..].join(" ");
            
            match find_redeem(client, &user_input).await {
                Ok(Some(rd)) => format_redeem_details(&rd),
                Ok(None) => format!("No redeem found matching '{}'", user_input),
                Err(e) => format!("Error: {}", e),
            }
        }
        
        "add" => {
            if args.len() < 3 {
                return "Usage: redeem add <rewardName> <cost> [--managed] [--offline] [--dynamic] [--input] [--prompt=\"prompt text\"]".to_string();
            }
            
            // Find where cost starts (first numeric argument)
            let mut cost_idx = None;
            for (i, arg) in args[1..].iter().enumerate() {
                if arg.parse::<i32>().is_ok() {
                    cost_idx = Some(i + 1);
                    break;
                }
            }
            
            let (reward_name, cost) = match cost_idx {
                Some(idx) => {
                    let name = args[1..idx].join(" ");
                    let cost = match args[idx].parse::<i32>() {
                        Ok(n) => n,
                        Err(_) => return "Cost must be an integer.".to_string(),
                    };
                    (name, cost)
                }
                None => return "Cost must be specified as an integer.".to_string(),
            };
            
            // Parse flags from remaining args
            let flag_start = cost_idx.unwrap() + 1;
            let mut is_managed = false;
            let mut active_offline = false;
            let mut dynamic_pricing = false;
            let mut is_input_required = false;
            let mut redeem_prompt_text = None;
            
            for flag in args.get(flag_start..).unwrap_or(&[]) {
                if let Some(prompt_text) = flag.strip_prefix("--prompt=") {
                    redeem_prompt_text = Some(prompt_text.to_string());
                } else {
                    match flag.to_lowercase().as_str() {
                        "--managed" => is_managed = true,
                        "--offline" => active_offline = true,
                        "--dynamic" => dynamic_pricing = true,
                        "--input" => is_input_required = true,
                        _ => {}
                    }
                }
            }
            
            // Create the redeem
            match RedeemCommands::create_redeem(
                client,
                "twitch-eventsub",
                &reward_name,
                None, // twitch_id
                None, // plugin_id
                cost,
                true, // is_enabled
                false, // is_paused
                false, // should_skip_request_queue
                is_managed,
                redeem_prompt_text.as_deref(),
                is_input_required,
                None, // command_name
            ).await {
                Ok(result) => {
                    let rd = &result.data.redeem;
                    
                    // If we need to set active_offline, update the redeem
                    if active_offline {
                        let mut updated_rd = rd.clone();
                        updated_rd.metadata.insert("active_offline".to_string(), "true".to_string());
                        match RedeemCommands::update_redeem(client, &rd.redeem_id, updated_rd).await {
                            Ok(_) => format!("Created redeem '{}' with cost {} (active offline).", rd.reward_name, rd.cost),
                            Err(e) => format!("Created redeem '{}' but failed to set offline mode: {}", rd.reward_name, e),
                        }
                    } else {
                        format!("Created redeem '{}' with cost {}.", rd.reward_name, rd.cost)
                    }
                }
                Err(e) => format!("Error creating redeem: {}", e),
            }
        }
        
        "enable" => handle_enable_disable(args, client, true).await,
        "disable" => handle_enable_disable(args, client, false).await,
        "pause" => handle_pause_unpause(args, client, true).await,
        "unpause" => handle_pause_unpause(args, client, false).await,
        
        "offline" => {
            if args.len() < 3 {
                return "Usage: redeem offline <redeemNameOrUuidOrNumber> <true|false>".to_string();
            }
            let redeem_input = args[1..args.len()-1].join(" ");
            let offline = match args.last().unwrap().to_lowercase().as_str() {
                "true" | "yes" | "1" => true,
                "false" | "no" | "0" => false,
                _ => return "Offline must be 'true' or 'false'.".to_string(),
            };
            
            match find_redeem(client, &redeem_input).await {
                Ok(Some(mut rd)) => {
                    rd.metadata.insert("active_offline".to_string(), offline.to_string());
                    match RedeemCommands::update_redeem(client, &rd.redeem_id, rd.clone()).await {
                        Ok(_) => format!("Updated offline status for '{}' to {}.", rd.reward_name, offline),
                        Err(e) => format!("Error updating offline status: {}", e),
                    }
                }
                Ok(None) => format!("Redeem '{}' not found.", redeem_input),
                Err(e) => format!("Error: {}", e),
            }
        }
        
        "setcost" => {
            if args.len() < 3 {
                return "Usage: redeem setcost <redeemNameOrUuidOrNumber> <newCost>".to_string();
            }
            
            let cost_str = args.last().unwrap();
            let new_cost = match cost_str.parse::<i32>() {
                Ok(n) => n,
                Err(_) => return "Cost must be an integer.".to_string(),
            };
            
            let redeem_input = args[1..args.len()-1].join(" ");
            
            match find_redeem(client, &redeem_input).await {
                Ok(Some(rd)) => {
                    match RedeemCommands::set_redeem_cost(client, "twitch-eventsub", &rd.reward_name, new_cost).await {
                        Ok(_) => format!("Updated cost for '{}' to {}.", rd.reward_name, new_cost),
                        Err(e) => format!("Error updating cost: {}", e),
                    }
                }
                Ok(None) => format!("Redeem '{}' not found.", redeem_input),
                Err(e) => format!("Error: {}", e),
            }
        }
        
        "setprompt" => {
            if args.len() < 3 {
                return "Usage: redeem setprompt <redeemNameOrUuidOrNumber> <promptText>".to_string();
            }
            
            // Find where prompt text starts - for now assume single word names for simplicity
            let prompt_start = 2;
            let redeem_input = args[1];
            let prompt_text = args[prompt_start..].join(" ");
            
            match find_redeem(client, redeem_input).await {
                Ok(Some(rd)) => {
                    match RedeemCommands::set_redeem_prompt(client, "twitch-eventsub", &rd.reward_name, &prompt_text).await {
                        Ok(_) => format!("Updated prompt for '{}'.", rd.reward_name),
                        Err(e) => format!("Error updating prompt: {}", e),
                    }
                }
                Ok(None) => format!("Redeem '{}' not found.", redeem_input),
                Err(e) => format!("Error: {}", e),
            }
        }
        
        "setplugin" => {
            if args.len() < 3 {
                return "Usage: redeem setplugin <redeemNameOrUuidOrNumber> <pluginId|builtin>".to_string();
            }
            
            let plugin_id = args.last().unwrap();
            let plugin_id = if plugin_id.eq_ignore_ascii_case("builtin") {
                ""
            } else {
                plugin_id
            };
            
            let redeem_input = args[1..args.len()-1].join(" ");
            
            match find_redeem(client, &redeem_input).await {
                Ok(Some(rd)) => {
                    match RedeemCommands::update_plugin(client, "twitch-eventsub", &rd.reward_name, plugin_id).await {
                        Ok(_) => format!(
                            "Updated plugin for '{}' to {}.",
                            rd.reward_name,
                            if plugin_id.is_empty() { "BUILTIN" } else { plugin_id }
                        ),
                        Err(e) => format!("Error updating plugin: {}", e),
                    }
                }
                Ok(None) => format!("Redeem '{}' not found.", redeem_input),
                Err(e) => format!("Error: {}", e),
            }
        }
        
        "setcommand" => {
            if args.len() < 3 {
                return "Usage: redeem setcommand <redeemNameOrUuidOrNumber> <commandName|none>".to_string();
            }
            
            let command_name = args.last().unwrap();
            let command_name = if command_name.eq_ignore_ascii_case("none") {
                ""
            } else {
                command_name
            };
            
            let redeem_input = args[1..args.len()-1].join(" ");
            
            match find_redeem(client, &redeem_input).await {
                Ok(Some(rd)) => {
                    match RedeemCommands::update_command(client, "twitch-eventsub", &rd.reward_name, command_name).await {
                        Ok(_) => format!(
                            "Updated command for '{}' to {}.",
                            rd.reward_name,
                            if command_name.is_empty() { "(none)" } else { command_name }
                        ),
                        Err(e) => format!("Error updating command: {}", e),
                    }
                }
                Ok(None) => format!("Redeem '{}' not found.", redeem_input),
                Err(e) => format!("Error: {}", e),
            }
        }
        
        "setinput" => {
            if args.len() < 3 {
                return "Usage: redeem setinput <redeemNameOrUuidOrNumber> <true|false>".to_string();
            }
            
            let input_str = args.last().unwrap();
            let input_required = match input_str.to_lowercase().as_str() {
                "true" | "yes" | "1" => true,
                "false" | "no" | "0" => false,
                _ => return "Input required must be 'true' or 'false'.".to_string(),
            };
            
            let redeem_input = args[1..args.len()-1].join(" ");
            
            match find_redeem(client, &redeem_input).await {
                Ok(Some(rd)) => {
                    match RedeemCommands::update_input_required(client, "twitch-eventsub", &rd.reward_name, input_required).await {
                        Ok(_) => format!("Updated input_required for '{}' to {}.", rd.reward_name, input_required),
                        Err(e) => format!("Error updating input_required: {}", e),
                    }
                }
                Ok(None) => format!("Redeem '{}' not found.", redeem_input),
                Err(e) => format!("Error: {}", e),
            }
        }
        
        "remove" => {
            if args.len() < 2 {
                return "Usage: redeem remove <redeemNameOrUuidOrNumber>".to_string();
            }
            
            let redeem_input = args[1..].join(" ");
            
            match find_redeem(client, &redeem_input).await {
                Ok(Some(rd)) => {
                    if rd.metadata.get("is_managed") != Some(&"true".to_string()) {
                        return format!("Cannot remove web-app managed redeem '{}'. Use Twitch dashboard.", rd.reward_name);
                    }
                    
                    println!("Are you sure you want to remove redeem '{}'? (y/n)", rd.reward_name);
                    print!("> ");
                    let _ = stdout().flush();
                    let mut confirm = String::new();
                    let _ = stdin().read_line(&mut confirm);
                    
                    if confirm.trim().eq_ignore_ascii_case("y") {
                        match RedeemCommands::delete_redeem(client, &rd.redeem_id).await {
                            Ok(_) => format!("Removed redeem '{}'.", rd.reward_name),
                            Err(e) => format!("Error removing redeem: {}", e),
                        }
                    } else {
                        "Removal cancelled.".to_string()
                    }
                }
                Ok(None) => format!("Redeem '{}' not found.", redeem_input),
                Err(e) => format!("Error: {}", e),
            }
        }
        
        "sync" => {
            match RedeemCommands::sync_redeems(client, "twitch-eventsub").await {
                Ok(result) => {
                    if result.data.added_count == 0 && result.data.updated_count == 0 && result.data.removed_count == 0 {
                        "Sync complete: No changes detected.".to_string()
                    } else {
                        format!(
                            "Sync complete: {} added, {} updated, {} removed.",
                            result.data.added_count,
                            result.data.updated_count,
                            result.data.removed_count
                        )
                    }
                }
                Err(e) => format!("Error syncing redeems: {}", e),
            }
        }
        
        _ => "Unknown redeem subcommand. Usage: redeem <list|info|add|enable|disable|pause|unpause|setcost|setprompt|setplugin|setcommand|setinput|remove|sync>".to_string(),
    }
}

// Helper function to handle enable/disable
async fn handle_enable_disable(args: &[&str], client: &GrpcClient, enable: bool) -> String {
    if args.len() < 2 {
        return format!("Usage: redeem {} <redeemNameOrUuidOrNumber>", args[0]);
    }
    
    let redeem_input = args[1..].join(" ");
    
    match find_redeem(client, &redeem_input).await {
        Ok(Some(rd)) => {
            match RedeemCommands::set_redeem_state(client, "twitch-eventsub", &rd.reward_name, Some(enable), None).await {
                Ok(_) => format!("{} redeem '{}'.", if enable { "Enabled" } else { "Disabled" }, rd.reward_name),
                Err(e) => format!("Error {} redeem: {}", if enable { "enabling" } else { "disabling" }, e),
            }
        }
        Ok(None) => format!("Redeem '{}' not found.", redeem_input),
        Err(e) => format!("Error: {}", e),
    }
}

// Helper function to handle pause/unpause
async fn handle_pause_unpause(args: &[&str], client: &GrpcClient, pause: bool) -> String {
    if args.len() < 2 {
        return format!("Usage: redeem {} <redeemNameOrUuidOrNumber>", args[0]);
    }
    
    let redeem_input = args[1..].join(" ");
    
    match find_redeem(client, &redeem_input).await {
        Ok(Some(rd)) => {
            if rd.metadata.get("is_managed") == Some(&"true".to_string()) {
                return format!("Cannot {} internally managed redeem '{}'. Use enable/disable instead.", 
                    if pause { "pause" } else { "unpause" }, rd.reward_name);
            }
            
            match RedeemCommands::set_redeem_state(client, "twitch-eventsub", &rd.reward_name, None, Some(pause)).await {
                Ok(_) => format!("{} redeem '{}'.", if pause { "Paused" } else { "Unpaused" }, rd.reward_name),
                Err(e) => format!("Error {} redeem: {}", if pause { "pausing" } else { "unpausing" }, e),
            }
        }
        Ok(None) => format!("Redeem '{}' not found.", redeem_input),
        Err(e) => format!("Error: {}", e),
    }
}

// Find a redeem by name, UUID, or number (for internal redeems)
async fn find_redeem(client: &GrpcClient, input: &str) -> Result<Option<Redeem>, String> {
    // First try to parse as UUID
    if let Ok(uuid) = input.parse::<Uuid>() {
        match RedeemCommands::get_redeem(client, &uuid.to_string()).await {
            Ok(result) => Ok(Some(result.data.redeem)),
            Err(e) => Err(format!("Error getting redeem: {}", e)),
        }
    } else if let Ok(number) = input.parse::<usize>() {
        // If it's a number, use it as an index for internal redeems
        match RedeemCommands::list_redeems(client, Some("twitch-eventsub"), false, 100).await {
            Ok(result) => {
                let internal_redeems: Vec<_> = result.data.redeems.into_iter()
                    .filter_map(|info| info.redeem)
                    .filter(|rd| rd.metadata.get("is_managed") == Some(&"true".to_string()))
                    .collect();
                
                if number > 0 && number <= internal_redeems.len() {
                    Ok(Some(internal_redeems[number - 1].clone()))
                } else {
                    Ok(None)
                }
            }
            Err(e) => Err(format!("Error listing redeems: {}", e)),
        }
    } else {
        // Try to find by name
        match RedeemCommands::find_redeem_by_name(client, "twitch-eventsub", input).await {
            Ok(redeem) => Ok(redeem),
            Err(e) => Err(format!("Error finding redeem: {}", e)),
        }
    }
}

// Format redeem details for display
fn format_redeem_details(rd: &Redeem) -> String {
    let is_paused = rd.metadata.get("is_paused") == Some(&"true".to_string());
    let status = if rd.is_active && !is_paused {
        "ACTIVE"
    } else if rd.is_active && is_paused {
        "PAUSED"
    } else {
        "DISABLED"
    };
    
    format!(
        "Redeem Details:\n\
         ID:                    {}\n\
         Name:                  {}\n\
         Cost:                  {}\n\
         Status:                {}\n\
         Dynamic Pricing:       {}\n\
         Active Offline:        {}\n\
         Is Managed:            {}\n\
         Input Required:        {}\n\
         Prompt:                {}\n\
         Plugin:                {}\n\
         Command:               {}\n\
         Twitch ID:             {}\n",
        rd.redeem_id,
        rd.reward_name,
        rd.cost,
        status,
        rd.is_dynamic,
        rd.metadata.get("active_offline").map(|s| s.as_str()).unwrap_or("false"),
        rd.metadata.get("is_managed").map(|s| s.as_str()).unwrap_or("false"),
        rd.metadata.get("input_required").map(|s| s.as_str()).unwrap_or("false"),
        rd.metadata.get("prompt").map(|s| s.as_str()).unwrap_or("-"),
        rd.metadata.get("plugin_id").map(|s| if s.is_empty() { "BUILTIN" } else { s }).unwrap_or("BUILTIN"),
        rd.metadata.get("command_name").map(|s| if s.is_empty() { "(none)" } else { s }).unwrap_or("(none)"),
        rd.metadata.get("twitch_id").map(|s| s.as_str()).unwrap_or("-"),
    )
}

// Format redeems as a table
fn format_table(redeems: &[Redeem], enumerate: bool) -> String {
    if redeems.is_empty() {
        return "(none)\n".to_string();
    }

    // Collect data: [#, Name, Cost, Actv, Offl, Input, Plugin, Command, UUID]
    let mut rows: Vec<Vec<String>> = Vec::new();
    for (idx, rd) in redeems.iter().enumerate() {
        let is_paused = rd.metadata.get("is_paused") == Some(&"true".to_string());
        let active_str = if rd.is_active && !is_paused {
            "true"
        } else if rd.is_active && is_paused {
            "paused"
        } else {
            "false"
        };
        
        let mut row = vec![];
        if enumerate {
            row.push(format!("{}", idx + 1));
        }
        row.extend(vec![
            rd.reward_name.clone(),
            format!("{:>4}", rd.cost),
            active_str.to_string(),
            rd.metadata.get("active_offline").map(|s| s.as_str()).unwrap_or("false").to_string(),
            rd.metadata.get("input_required").map(|s| s.as_str()).unwrap_or("false").to_string(),
            rd.metadata.get("plugin_id").map(|s| if s.is_empty() { "-" } else { s }).unwrap_or("-").to_string(),
            rd.metadata.get("command_name").map(|s| if s.is_empty() { "-" } else { s }).unwrap_or("-").to_string(),
            rd.redeem_id.clone(),
        ]);
        rows.push(row);
    }

    // Determine column widths
    let headers = if enumerate {
        vec!["#", "Name", "Cost", "Actv", "Offl", "Input", "Plugin", "Command", "UUID"]
    } else {
        vec!["Name", "Cost", "Actv", "Offl", "Input", "Plugin", "Command", "UUID"]
    };
    
    let mut col_widths: Vec<usize> = headers.iter().map(|h| h.len()).collect();
    
    for row in &rows {
        for (i, cell) in row.iter().enumerate() {
            col_widths[i] = col_widths[i].max(cell.len());
        }
    }

    let mut out = String::new();
    
    // Format header row
    for (i, header) in headers.iter().enumerate() {
        if i > 0 {
            out.push_str("  ");
        }
        out.push_str(&format!("{:<width$}", header, width = col_widths[i]));
    }
    out.push('\n');

    // Format data rows
    for row in rows {
        for (i, cell) in row.iter().enumerate() {
            if i > 0 {
                out.push_str("  ");
            }
            // Right-align cost column
            if (enumerate && i == 2) || (!enumerate && i == 1) {
                out.push_str(&format!("{:>width$}", cell, width = col_widths[i]));
            } else {
                out.push_str(&format!("{:<width$}", cell, width = col_widths[i]));
            }
        }
        out.push('\n');
    }

    out
}