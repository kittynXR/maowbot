// OSC command adapter for TUI
use maowbot_common_ui::{GrpcClient, commands::osc::OscCommands};
use std::sync::Arc;
use crate::tui_module::TuiModule;

pub async fn handle_osc_command(
    args: &[&str],
    client: &GrpcClient,
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
  osc toggle <subcommand>         - Manage OSC toggle triggers for redeems
    toggle list                   - Show all configured OSC toggles
    toggle test <param> <value>   - Test sending OSC parameter
    toggle active                 - Show currently active toggles
  osc set <subcommand>            - Configure OSC destinations
    set vrcdest <ip:port>         - Set VRChat OSC destination (default: 127.0.0.1:9000)
    set robodest <ip:port>        - Set Robot OSC destination
"#.to_string();
    }
    
    match args[0] {
        "start" => {
            match OscCommands::start(client).await {
                Ok(_) => "OSC started.".to_string(),
                Err(e) => format!("Error => {}", e),
            }
        }
        "stop" => {
            match OscCommands::stop(client).await {
                Ok(_) => "OSC stopped.".to_string(),
                Err(e) => format!("Error => {}", e),
            }
        }
        "restart" => {
            match OscCommands::restart(client).await {
                Ok(_) => "OSC restarted.".to_string(),
                Err(e) => format!("Error => {}", e),
            }
        }
        "chatbox" => {
            // If there's more than 1 sub-arg, treat it as a single message
            if args.len() > 1 {
                let message = args[1..].join(" ");
                match OscCommands::send_chatbox(client, &message).await {
                    Ok(_) => format!("(sent) {}", message),
                    Err(e) => format!("Error => {}", e),
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
            match OscCommands::get_status(client).await {
                Ok(stat) => {
                    let mut status = format!(
                        "OSC running={} port={:?}, OSCQuery={} http_port={:?}",
                        stat.is_running,
                        stat.listening_port,
                        stat.is_oscquery_running,
                        stat.oscquery_port
                    );
                    
                    // Get configured destinations using config service
                    match client.config.clone()
                        .get_config(maowbot_proto::maowbot::services::GetConfigRequest {
                            key: "osc_vrchat_dest".to_string(),
                            include_metadata: false,
                        })
                        .await
                    {
                        Ok(response) => {
                            if let Some(config) = response.into_inner().config {
                                status.push_str(&format!("\nVRChat destination: {}", config.value));
                            } else {
                                status.push_str("\nVRChat destination: default (127.0.0.1:9000)");
                            }
                        }
                        Err(_) => {
                            status.push_str("\nVRChat destination: default (127.0.0.1:9000)");
                        }
                    }
                    
                    match client.config.clone()
                        .get_config(maowbot_proto::maowbot::services::GetConfigRequest {
                            key: "osc_robot_dest".to_string(),
                            include_metadata: false,
                        })
                        .await
                    {
                        Ok(response) => {
                            if let Some(config) = response.into_inner().config {
                                status.push_str(&format!("\nRobot destination: {}", config.value));
                            } else {
                                status.push_str("\nRobot destination: not configured");
                            }
                        }
                        Err(_) => {
                            status.push_str("\nRobot destination: not configured");
                        }
                    }
                    
                    status
                }
                Err(e) => format!("Error => {}", e),
            }
        }
        "discover" => {
            match OscCommands::discover_peers(client).await {
                Ok(list) if list.is_empty() => "No local OSCQuery services discovered.".to_string(),
                Ok(list) => {
                    let lines = list.into_iter()
                        .map(|name| format!(" - {name}"))
                        .collect::<Vec<_>>()
                        .join("\n");
                    format!("Discovered:\n{}", lines)
                }
                Err(e) => format!("Error => {}", e),
            }
        }
        "toggle" => {
            if args.len() < 2 {
                return r#"Usage:
  osc toggle list                           - List all configured OSC toggles
  osc toggle test <param> <type> <value> [duration] - Test an OSC parameter
    Types: bool, int, float
    Duration: optional, in seconds (default: permanent)
  osc toggle active                         - Show currently active toggles"#.to_string();
            }
            
            match args[1] {
                "list" => {
                    match OscCommands::list_triggers_with_redeems(client).await {
                        Ok(result) => {
                            if result.triggers.is_empty() {
                                "No OSC triggers configured.".to_string()
                            } else {
                                let mut output = String::from("OSC Toggle Configurations\n");
                                output.push_str("========================\n\n");
                                output.push_str("ID  | Redeem Name      | Parameter       | Type  | On    | Off   | Duration | Cooldown | Enabled\n");
                                output.push_str("----|------------------|-----------------|-------|-------|-------|----------|----------|---------\n");
                                
                                for (trigger, redeem_name) in result.triggers {
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
                        Err(e) => format!("Error listing triggers: {}", e),
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
                                    match OscCommands::send_avatar_parameter_bool(client, param, bool_val).await {
                                        Ok(_) => {
                                            let msg = format!("Sent OSC: {} = {} (bool)", param, bool_val);
                                            if let Some(dur) = duration {
                                                // Schedule toggle off
                                                let param_name = param.to_string();
                                                let client_clone = client.clone();
                                                tokio::spawn(async move {
                                                    tokio::time::sleep(tokio::time::Duration::from_secs(dur)).await;
                                                    let _ = OscCommands::send_avatar_parameter_bool(&client_clone, &param_name, !bool_val).await;
                                                });
                                                format!("{} - will toggle back in {}s", msg, dur)
                                            } else {
                                                msg
                                            }
                                        }
                                        Err(e) => format!("Error sending OSC: {}", e),
                                    }
                                }
                                Err(_) => "Invalid boolean value. Use 'true' or 'false'.".to_string(),
                            }
                        }
                        "int" => {
                            match value.parse::<i32>() {
                                Ok(int_val) => {
                                    match OscCommands::send_avatar_parameter_int(client, param, int_val).await {
                                        Ok(_) => format!("Sent OSC: {} = {} (int)", param, int_val),
                                        Err(e) => format!("Error sending OSC: {}", e),
                                    }
                                }
                                Err(_) => "Invalid integer value.".to_string(),
                            }
                        }
                        "float" => {
                            match value.parse::<f32>() {
                                Ok(float_val) => {
                                    match OscCommands::send_avatar_parameter_float(client, param, float_val).await {
                                        Ok(_) => format!("Sent OSC: {} = {} (float)", param, float_val),
                                        Err(e) => format!("Error sending OSC: {}", e),
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
                    match OscCommands::list_active_toggles(client, None).await {
                        Ok(toggles) => {
                            if toggles.is_empty() {
                                "No active OSC toggles.".to_string()
                            } else {
                                let mut output = String::from("Active OSC Toggles\n");
                                output.push_str("==================\n\n");
                                output.push_str("ID  | Trigger | User ID | Activated At        | Expires At\n");
                                output.push_str("----|---------|---------|---------------------|---------------------\n");
                                
                                for toggle in toggles {
                                    let user_id_short = if toggle.user_id.len() > 7 {
                                        &toggle.user_id[..7]
                                    } else {
                                        &toggle.user_id
                                    };
                                    output.push_str(&format!(
                                        "{:<3} | {:<7} | {:<7} | {} | {}\n",
                                        toggle.id,
                                        toggle.trigger_id,
                                        user_id_short,
                                        toggle.activated_at,
                                        toggle.expires_at.unwrap_or_else(|| "Never".to_string())
                                    ));
                                }
                                output
                            }
                        }
                        Err(e) => format!("Error listing active toggles: {}", e),
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
                                
                                let request = maowbot_proto::maowbot::services::SetConfigRequest {
                                    key: "osc_vrchat_dest".to_string(),
                                    value: normalized_dest.clone(),
                                    metadata: None,
                                    validate_only: false,
                                };
                                
                                match client.config.clone().set_config(request).await {
                                    Ok(_) => {
                                        // Provide a warning for non-local IPs
                                        if !ip.starts_with("127.") && !ip.starts_with("192.168.") && !ip.starts_with("10.") && !ip.starts_with("172.") {
                                            format!("VRChat OSC destination set to: {}\nWarning: {} appears to be a public IP. Make sure VRChat is actually listening on this address.", normalized_dest, ip)
                                        } else {
                                            format!("VRChat OSC destination set to: {}", normalized_dest)
                                        }
                                    }
                                    Err(e) => format!("Error setting VRChat destination: {}", e),
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
                                
                                let request = maowbot_proto::maowbot::services::SetConfigRequest {
                                    key: "osc_robot_dest".to_string(),
                                    value: normalized_dest.clone(),
                                    metadata: None,
                                    validate_only: false,
                                };
                                
                                match client.config.clone().set_config(request).await {
                                    Ok(_) => {
                                        // Provide a warning for non-local IPs
                                        if !ip.starts_with("127.") && !ip.starts_with("192.168.") && !ip.starts_with("10.") && !ip.starts_with("172.") {
                                            format!("Robot OSC destination set to: {}\nWarning: {} appears to be a public IP. Make sure the robot instance is actually listening on this address.", normalized_dest, ip)
                                        } else {
                                            format!("Robot OSC destination set to: {}", normalized_dest)
                                        }
                                    }
                                    Err(e) => format!("Error setting Robot destination: {}", e),
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