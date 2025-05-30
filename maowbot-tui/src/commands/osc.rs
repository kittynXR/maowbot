//! maowbot-tui/src/commands/osc.rs
//!
//! Handles "osc" commands in the TUI, including:
//!   osc start
//!   osc stop
//!   osc restart
//!   osc chatbox <message>
//!     (if no <message>, open a REPL-like chat loop until /quit)
//!   osc status
//!   osc discover
//!   osc raw
//!

use std::sync::Arc;
use maowbot_common::traits::api::BotApi;
use crate::tui_module::TuiModule;

/// Dispatches the "osc" subcommands.
pub async fn handle_osc_command(
    args: &[&str],
    bot_api: &Arc<dyn BotApi>,
    tui_module: &Arc<TuiModule>,
) -> String {
    if args.is_empty() {
        return r#"Usage:
  osc start                       - Start OSC service
  osc stop                        - Stop OSC service
  osc restart                     - Restart OSC service
  osc chatbox [message...]        - Send message to VRChat chatbox (interactive if no message)
  osc status                      - Show OSC service status
  osc discover                    - Discover local OSCQuery services
  osc raw                         - Start raw OSC packet listener
  osc toggle <subcommand>         - Manage OSC toggle triggers for redeems
    toggle list                   - Show SQL commands for managing triggers
    toggle test <param> <value>   - Test sending OSC parameter
    toggle quick                  - Quick reference for VRChat parameters
    toggle examples               - Example configurations
  osc set <subcommand>            - Configure OSC destinations
    set vrcdest <ip:port>         - Set VRChat OSC destination (default: 127.0.0.1:9000)
    set robodest <ip:port>        - Set Robot OSC destination
"#.to_string();
    }
    match args[0] {
        "start" => {
            match bot_api.osc_start().await {
                Ok(_) => "OSC started.".to_string(),
                Err(e) => format!("Error => {:?}", e),
            }
        }
        "stop" => {
            match bot_api.osc_stop().await {
                Ok(_) => "OSC stopped.".to_string(),
                Err(e) => format!("Error => {:?}", e),
            }
        }
        "restart" => {
            match bot_api.osc_restart().await {
                Ok(_) => "OSC restarted.".to_string(),
                Err(e) => format!("Error => {:?}", e),
            }
        }
        "chatbox" => {
            // If there's more than 1 sub-arg, treat it as a single message
            if args.len() > 1 {
                let message = args[1..].join(" ");
                match bot_api.osc_chatbox(&message).await {
                    Ok(_) => format!("(sent) {}", message),
                    Err(e) => format!("Error => {:?}", e),
                }
            } else {
                // No argument => go into chatbox REPL mode
                let mut st = tui_module.osc_state.lock().unwrap();
                st.is_in_chat_mode = true;
                drop(st);
                "Entering OSC chatbox mode. Type /quit to exit.".to_string()
            }
        }
        "status" => {
            match bot_api.osc_status().await {
                Ok(stat) => {
                    let mut status = format!(
                        "OSC running={} port={:?}, OSCQuery={} http_port={:?}",
                        stat.is_running,
                        stat.listening_port,
                        stat.is_oscquery_running,
                        stat.oscquery_port
                    );
                    
                    // Get configured destinations
                    if let Ok(Some(vrchat_dest)) = bot_api.get_bot_config_value("osc_vrchat_dest").await {
                        status.push_str(&format!("\nVRChat destination: {}", vrchat_dest));
                    } else {
                        status.push_str("\nVRChat destination: default (127.0.0.1:9000)");
                    }
                    
                    if let Ok(Some(robot_dest)) = bot_api.get_bot_config_value("osc_robot_dest").await {
                        status.push_str(&format!("\nRobot destination: {}", robot_dest));
                    } else {
                        status.push_str("\nRobot destination: not configured");
                    }
                    
                    status
                }
                Err(e) => format!("Error => {:?}", e),
            }
        }
        "discover" => {
            match bot_api.osc_discover_peers().await {
                Ok(list) if list.is_empty() => "No local OSCQuery services discovered.".to_string(),
                Ok(list) => {
                    let lines = list.into_iter()
                        .map(|name| format!(" - {name}"))
                        .collect::<Vec<_>>()
                        .join("\n");
                    format!("Discovered:\n{}", lines)
                }
                Err(e) => format!("Error => {:?}", e),
            }
        }
        "raw" => {
            // Start the OSC raw packet listener
            println!("Starting OSC raw packet monitor. Press Ctrl+C to stop.");

            match bot_api.osc_take_raw_receiver().await {
                Ok(Some(mut rx)) => {
                    // Store the handle so it can be aborted during shutdown
                    let task_handle = tokio::spawn(async move {
                        println!("Raw OSC packet monitoring active.");
                        println!("Waiting for incoming OSC packets...");

                        while let Some(packet) = rx.recv().await {
                            match packet {
                                rosc::OscPacket::Message(msg) => {
                                    println!("OSC Message: addr={}, args={:?}", msg.addr, msg.args);
                                }
                                rosc::OscPacket::Bundle(bundle) => {
                                    println!("OSC Bundle: time={:?}, {} messages",
                                             bundle.timetag, bundle.content.len());
                                    for (i, content) in bundle.content.iter().enumerate() {
                                        println!("  [{}]: {:?}", i, content);
                                    }
                                }
                            }
                        }

                        println!("OSC raw packet monitor stopped.");
                    });

                    // Store the task handle in a global registry or similar
                    // For now we let it run but in a proper implementation we'd
                    // want to keep track of this to abort it during shutdown

                    "OSC raw packet monitor started. Messages will appear in console.".to_string()
                },
                Ok(None) => "No OSC receiver available. Try starting OSC first.".to_string(),
                Err(e) => format!("Error getting OSC receiver: {:?}", e),
            }
        },
        "toggle" => {
            if args.len() < 2 {
                return r#"Usage:
  osc toggle list                           - List all configured OSC toggles
  osc toggle test <param> <type> <value> [duration] - Test an OSC parameter
    Types: bool, int, float
    Duration: optional, in seconds (default: permanent)
  osc toggle create <redeem_id> <param> <type> <on> <off> [duration] - Create new trigger
  osc toggle update <trigger_id> <field> <value> - Update existing trigger
  osc toggle delete <trigger_id>            - Delete a trigger
  osc toggle active                         - Show currently active toggles"#.to_string();
            }
            
            match args[1] {
                "list" => {
                    match bot_api.osc_list_triggers_with_redeems().await {
                        Ok(triggers) => {
                            if triggers.is_empty() {
                                "No OSC triggers configured.".to_string()
                            } else {
                                let mut output = String::from("OSC Toggle Configurations\n");
                                output.push_str("========================\n\n");
                                output.push_str("ID  | Redeem Name      | Parameter       | Type  | On    | Off   | Duration | Cooldown | Enabled\n");
                                output.push_str("----|------------------|-----------------|-------|-------|-------|----------|----------|---------\n");
                                
                                for (trigger, redeem_name) in triggers {
                                    output.push_str(&format!(
                                        "{:<3} | {:<16} | {:<15} | {:<5} | {:<5} | {:<5} | {:<8} | {:<8} | {}\n",
                                        trigger.id,
                                        if redeem_name.len() > 16 { &redeem_name[..16] } else { &redeem_name },
                                        if trigger.parameter_name.len() > 15 { &trigger.parameter_name[..15] } else { &trigger.parameter_name },
                                        &trigger.parameter_type,
                                        &trigger.on_value,
                                        &trigger.off_value,
                                        trigger.duration_seconds.map_or("perm".to_string(), |d| format!("{}s", d)),
                                        format!("{}s", trigger.cooldown_seconds),
                                        if trigger.enabled { "Yes" } else { "No" }
                                    ));
                                }
                                output
                            }
                        }
                        Err(e) => format!("Error listing triggers: {:?}", e),
                    }
                }
                "test" => {
                    if args.len() < 5 {
                        return "Usage: osc toggle test <parameter> <type> <value> [duration]\n  Types: bool, int, float\n  Duration: optional seconds (e.g., 30)".to_string();
                    }
                    let param = args[2];
                    let param_type = args[3];
                    let value = args[4];
                    let duration = args.get(5).and_then(|s| s.parse::<u64>().ok());
                    
                    let result = match param_type {
                        "bool" => {
                            match value.parse::<bool>() {
                                Ok(bool_val) => {
                                    match bot_api.osc_send_avatar_parameter_bool(param, bool_val).await {
                                        Ok(_) => {
                                            let msg = format!("Sent OSC: {} = {} (bool)", param, bool_val);
                                            if let Some(dur) = duration {
                                                // Schedule toggle off
                                                let param_name = param.to_string();
                                                let api_clone = bot_api.clone();
                                                tokio::spawn(async move {
                                                    tokio::time::sleep(tokio::time::Duration::from_secs(dur)).await;
                                                    let _ = api_clone.osc_send_avatar_parameter_bool(&param_name, !bool_val).await;
                                                });
                                                format!("{} - will toggle back in {}s", msg, dur)
                                            } else {
                                                msg
                                            }
                                        }
                                        Err(e) => format!("Error sending OSC: {:?}", e),
                                    }
                                }
                                Err(_) => "Invalid boolean value. Use 'true' or 'false'.".to_string(),
                            }
                        }
                        "int" => {
                            match value.parse::<i32>() {
                                Ok(int_val) => {
                                    match bot_api.osc_send_avatar_parameter_int(param, int_val).await {
                                        Ok(_) => format!("Sent OSC: {} = {} (int)", param, int_val),
                                        Err(e) => format!("Error sending OSC: {:?}", e),
                                    }
                                }
                                Err(_) => "Invalid integer value.".to_string(),
                            }
                        }
                        "float" => {
                            match value.parse::<f32>() {
                                Ok(float_val) => {
                                    match bot_api.osc_send_avatar_parameter_float(param, float_val).await {
                                        Ok(_) => format!("Sent OSC: {} = {} (float)", param, float_val),
                                        Err(e) => format!("Error sending OSC: {:?}", e),
                                    }
                                }
                                Err(_) => "Invalid float value.".to_string(),
                            }
                        }
                        _ => "Invalid type. Use 'bool', 'int', or 'float'.".to_string(),
                    };
                    
                    result
                }
                "active" => {
                    match bot_api.osc_list_active_toggles(None).await {
                        Ok(toggles) => {
                            if toggles.is_empty() {
                                "No active OSC toggles.".to_string()
                            } else {
                                let mut output = String::from("Active OSC Toggles\n");
                                output.push_str("==================\n\n");
                                output.push_str("ID  | Trigger | User ID | Activated At        | Expires At\n");
                                output.push_str("----|---------|---------|---------------------|---------------------\n");
                                
                                for toggle in toggles {
                                    output.push_str(&format!(
                                        "{:<3} | {:<7} | {:<7} | {} | {}\n",
                                        toggle.id,
                                        toggle.trigger_id,
                                        &toggle.user_id.to_string()[..7],
                                        toggle.activated_at.format("%Y-%m-%d %H:%M:%S"),
                                        toggle.expires_at.map_or("Never".to_string(), |e| e.format("%Y-%m-%d %H:%M:%S").to_string())
                                    ));
                                }
                                output
                            }
                        }
                        Err(e) => format!("Error listing active toggles: {:?}", e),
                    }
                }
                "create" => {
                    if args.len() < 7 {
                        return "Usage: osc toggle create <redeem_id> <parameter> <type> <on_value> <off_value> [duration]\n  Example: osc toggle create 550e8400-e29b-41d4-a716-446655440000 /avatar/parameters/Wings bool true false 30".to_string();
                    }
                    
                    let redeem_id = match uuid::Uuid::parse_str(args[2]) {
                        Ok(id) => id,
                        Err(_) => {
                            // Try to list redeems to help the user
                            return match bot_api.list_redeems("twitch").await {
                                Ok(redeems) => {
                                    let mut output = format!("Invalid UUID format. Available redeems:\n");
                                    for redeem in redeems {
                                        output.push_str(&format!("  {} - {}\n", redeem.redeem_id, redeem.reward_name));
                                    }
                                    output.push_str("\nUsage: osc toggle create <redeem_id> <parameter> <type> <on_value> <off_value> [duration]");
                                    output
                                }
                                Err(_) => "Invalid UUID format for redeem_id.".to_string(),
                            };
                        }
                    };
                    
                    let parameter_name = args[3].to_string();
                    let parameter_type = args[4].to_string();
                    let on_value = args[5].to_string();
                    let off_value = args[6].to_string();
                    let duration = args.get(7).and_then(|s| s.parse::<i32>().ok());
                    
                    // Validate parameter type
                    if !["bool", "int", "float"].contains(&parameter_type.as_str()) {
                        return "Invalid parameter type. Use 'bool', 'int', or 'float'.".to_string();
                    }
                    
                    // Validate values based on type
                    let validation_result = match parameter_type.as_str() {
                        "bool" => {
                            if on_value.parse::<bool>().is_err() || off_value.parse::<bool>().is_err() {
                                Err("Invalid boolean values. Use 'true' or 'false'.".to_string())
                            } else {
                                Ok(())
                            }
                        }
                        "int" => {
                            if on_value.parse::<i32>().is_err() || off_value.parse::<i32>().is_err() {
                                Err("Invalid integer values.".to_string())
                            } else {
                                Ok(())
                            }
                        }
                        "float" => {
                            if on_value.parse::<f32>().is_err() || off_value.parse::<f32>().is_err() {
                                Err("Invalid float values.".to_string())
                            } else {
                                Ok(())
                            }
                        }
                        _ => unreachable!(),
                    };
                    
                    if let Err(e) = validation_result {
                        return e;
                    }
                    
                    let trigger = maowbot_common::models::osc_toggle::OscTrigger {
                        id: 0, // Will be assigned by database
                        redeem_id,
                        parameter_name: parameter_name.clone(),
                        parameter_type: parameter_type.clone(),
                        on_value: on_value.clone(),
                        off_value: off_value.clone(),
                        duration_seconds: duration,
                        cooldown_seconds: 0, // Default cooldown
                        enabled: true,
                        created_at: chrono::Utc::now(),
                        updated_at: chrono::Utc::now(),
                    };
                    
                    match bot_api.osc_create_trigger(trigger).await {
                        Ok(created) => {
                            format!("Created OSC trigger #{} for parameter {} ({})", 
                                created.id, parameter_name,
                                if let Some(d) = duration {
                                    format!("{}s duration", d)
                                } else {
                                    "permanent".to_string()
                                }
                            )
                        }
                        Err(e) => format!("Error creating trigger: {:?}", e),
                    }
                }
                "update" => {
                    if args.len() < 5 {
                        return "Usage: osc toggle update <trigger_id> <field> <value>\n  Fields: parameter_name, parameter_type, on_value, off_value, duration_seconds, cooldown_seconds, enabled\n  Example: osc toggle update 3 duration_seconds 60".to_string();
                    }
                    
                    let trigger_id = match args[2].parse::<i32>() {
                        Ok(id) => id,
                        Err(_) => return "Invalid trigger ID. Must be a number.".to_string(),
                    };
                    
                    let field = args[3];
                    let value = args[4];
                    
                    // First, fetch the existing trigger
                    let triggers = match bot_api.osc_list_triggers_with_redeems().await {
                        Ok(triggers) => triggers,
                        Err(e) => return format!("Error fetching trigger: {:?}", e),
                    };
                    
                    let (mut trigger, _) = match triggers.into_iter().find(|(t, _)| t.id == trigger_id) {
                        Some(t) => t,
                        None => return format!("Trigger #{} not found.", trigger_id),
                    };
                    
                    // Update the specified field
                    match field {
                        "parameter_name" => trigger.parameter_name = value.to_string(),
                        "parameter_type" => {
                            if !["bool", "int", "float"].contains(&value) {
                                return "Invalid parameter type. Use 'bool', 'int', or 'float'.".to_string();
                            }
                            trigger.parameter_type = value.to_string();
                        }
                        "on_value" => trigger.on_value = value.to_string(),
                        "off_value" => trigger.off_value = value.to_string(),
                        "duration_seconds" => {
                            trigger.duration_seconds = if value == "null" || value == "0" {
                                None
                            } else {
                                match value.parse::<i32>() {
                                    Ok(d) => Some(d),
                                    Err(_) => return "Invalid duration. Must be a number or 'null'.".to_string(),
                                }
                            };
                        }
                        "cooldown_seconds" => {
                            match value.parse::<i32>() {
                                Ok(c) => trigger.cooldown_seconds = c,
                                Err(_) => return "Invalid cooldown. Must be a number.".to_string(),
                            }
                        }
                        "enabled" => {
                            match value.parse::<bool>() {
                                Ok(e) => trigger.enabled = e,
                                Err(_) => return "Invalid enabled value. Use 'true' or 'false'.".to_string(),
                            }
                        }
                        _ => return format!("Unknown field '{}'. Valid fields: parameter_name, parameter_type, on_value, off_value, duration_seconds, cooldown_seconds, enabled", field),
                    }
                    
                    trigger.updated_at = chrono::Utc::now();
                    
                    match bot_api.osc_update_trigger(trigger).await {
                        Ok(_) => format!("Updated trigger #{} - {} set to '{}'", trigger_id, field, value),
                        Err(e) => format!("Error updating trigger: {:?}", e),
                    }
                }
                "delete" => {
                    if args.len() < 3 {
                        return "Usage: osc toggle delete <trigger_id>".to_string();
                    }
                    
                    let trigger_id = match args[2].parse::<i32>() {
                        Ok(id) => id,
                        Err(_) => return "Invalid trigger ID. Must be a number.".to_string(),
                    };
                    
                    match bot_api.osc_delete_trigger(trigger_id).await {
                        Ok(_) => format!("Deleted OSC trigger #{}", trigger_id),
                        Err(e) => format!("Error deleting trigger: {:?}", e),
                    }
                }
                _ => "Unknown toggle subcommand. Use 'osc toggle' for help.".to_string(),
            }
        },
        "set" => {
            if args.len() < 2 {
                return r#"Usage:
  osc set vrcdest <ip:port>   - Set VRChat OSC destination (default: 127.0.0.1:9000)
  osc set robodest <ip:port>  - Set Robot OSC destination"#.to_string();
            }
            
            match args[1] {
                "vrcdest" => {
                    if args.len() < 3 {
                        return "Usage: osc set vrcdest <ip:port>\nExample: osc set vrcdest 192.168.1.100:9000".to_string();
                    }
                    
                    let dest = args[2];
                    // Validate the format
                    if let Some((ip, port)) = dest.split_once(':') {
                        if let Ok(_port_num) = port.parse::<u16>() {
                            // Validate IP format (basic check)
                            let ip_parts: Vec<&str> = ip.split('.').collect();
                            if ip == "localhost" || (ip_parts.len() == 4 && ip_parts.iter().all(|p| p.parse::<u8>().is_ok())) {
                                // Convert localhost to 127.0.0.1 for consistency
                                let normalized_dest = if ip == "localhost" {
                                    format!("127.0.0.1:{}", port)
                                } else {
                                    dest.to_string()
                                };
                                
                                match bot_api.set_bot_config_value("osc_vrchat_dest", &normalized_dest).await {
                                    Ok(_) => {
                                        // Provide a warning for non-local IPs
                                        if !ip.starts_with("127.") && !ip.starts_with("192.168.") && !ip.starts_with("10.") && !ip.starts_with("172.") {
                                            format!("VRChat OSC destination set to: {}\nWarning: {} appears to be a public IP. Make sure VRChat is actually listening on this address.", normalized_dest, ip)
                                        } else {
                                            format!("VRChat OSC destination set to: {}", normalized_dest)
                                        }
                                    }
                                    Err(e) => format!("Error setting VRChat destination: {:?}", e),
                                }
                            } else {
                                "Invalid IP address format. Use format: x.x.x.x:port".to_string()
                            }
                        } else {
                            "Invalid port number. Port must be between 0-65535.".to_string()
                        }
                    } else {
                        "Invalid format. Use: ip:port (e.g., 127.0.0.1:9000)".to_string()
                    }
                }
                "robodest" => {
                    if args.len() < 3 {
                        return "Usage: osc set robodest <ip:port>\nExample: osc set robodest 192.168.1.100:9100".to_string();
                    }
                    
                    let dest = args[2];
                    // Validate the format
                    if let Some((ip, port)) = dest.split_once(':') {
                        if let Ok(_port_num) = port.parse::<u16>() {
                            // Validate IP format (basic check)
                            let ip_parts: Vec<&str> = ip.split('.').collect();
                            if ip == "localhost" || (ip_parts.len() == 4 && ip_parts.iter().all(|p| p.parse::<u8>().is_ok())) {
                                // Convert localhost to 127.0.0.1 for consistency
                                let normalized_dest = if ip == "localhost" {
                                    format!("127.0.0.1:{}", port)
                                } else {
                                    dest.to_string()
                                };
                                
                                match bot_api.set_bot_config_value("osc_robot_dest", &normalized_dest).await {
                                    Ok(_) => {
                                        // Provide a warning for non-local IPs
                                        if !ip.starts_with("127.") && !ip.starts_with("192.168.") && !ip.starts_with("10.") && !ip.starts_with("172.") {
                                            format!("Robot OSC destination set to: {}\nWarning: {} appears to be a public IP. Make sure the robot instance is actually listening on this address.", normalized_dest, ip)
                                        } else {
                                            format!("Robot OSC destination set to: {}", normalized_dest)
                                        }
                                    }
                                    Err(e) => format!("Error setting Robot destination: {:?}", e),
                                }
                            } else {
                                "Invalid IP address format. Use format: x.x.x.x:port".to_string()
                            }
                        } else {
                            "Invalid port number. Port must be between 0-65535.".to_string()
                        }
                    } else {
                        "Invalid format. Use: ip:port (e.g., 127.0.0.1:9100)".to_string()
                    }
                }
                _ => "Unknown set subcommand. Use 'osc set' for help.".to_string(),
            }
        },
        _ => "Unknown subcommand. Type 'osc' for help.".to_string(),
    }
}