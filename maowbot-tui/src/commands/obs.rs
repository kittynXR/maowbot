use maowbot_common_ui::commands::obs::ObsCommands;
use maowbot_common_ui::GrpcClient;

/// OBS command - manages OBS WebSocket connections and controls
pub async fn handle_obs_command(args: &[&str], client: &GrpcClient) -> String {
    if args.is_empty() {
        return "Usage: obs <subcommand> [options]\n\
                Subcommands:\n  \
                configure <instance> [--host <ip>] [--port <port>] [--ssl] [--no-ssl] [--password <pass>] [--no-password]\n  \
                list <scenes|sources|instances> [instance]\n  \
                select <scene|source> <name|number> [instance]\n  \
                source <hide|show|refresh> [name|number] [instance]\n  \
                start <stream|recording> [instance]\n  \
                stop <stream|recording> [instance]\n  \
                status [instance]".to_string();
    }

    match args[0] {
        "configure" => handle_configure_command(args, client).await,
        "instance" => handle_instance_config(args, client).await,
        "list" => handle_list_command(args, client).await,
        "select" => handle_select_command(args, client).await,
        "source" => handle_source_command(args, client).await,
        "start" => handle_start_command(args, client).await,
        "stop" => handle_stop_command(args, client).await,
        "status" => handle_status_command(args, client).await,
        _ => format!("Unknown OBS subcommand: {}", args[0]),
    }
}

async fn handle_configure_command(args: &[&str], client: &GrpcClient) -> String {
    if args.len() < 2 {
        return "Usage: obs configure <instance> [options]\n\
                Options:\n  \
                --host <ip>         Set host IP address\n  \
                --port <port>       Set port number\n  \
                --ssl               Enable SSL\n  \
                --no-ssl            Disable SSL\n  \
                --password <pass>   Set password\n  \
                --no-password       Disable password authentication".to_string();
    }
    
    let instance_num = match args[1].parse::<u32>() {
        Ok(n) => n,
        Err(_) => return "Invalid instance number".to_string(),
    };
    
    // Parse arguments
    let mut i = 2;
    let mut updates = Vec::new();
    
    while i < args.len() {
        match args[i] {
            "--host" => {
                if i + 1 < args.len() {
                    updates.push(("host", args[i + 1]));
                    i += 2;
                } else {
                    return "Missing value for --host".to_string();
                }
            }
            "--port" => {
                if i + 1 < args.len() {
                    updates.push(("port", args[i + 1]));
                    i += 2;
                } else {
                    return "Missing value for --port".to_string();
                }
            }
            "--ssl" => {
                updates.push(("ssl", "true"));
                i += 1;
            }
            "--no-ssl" => {
                updates.push(("ssl", "false"));
                i += 1;
            }
            "--password" => {
                if i + 1 < args.len() {
                    updates.push(("password", args[i + 1]));
                    i += 2;
                } else {
                    return "Missing value for --password".to_string();
                }
            }
            "--no-password" => {
                updates.push(("use_password", "false"));
                i += 1;
            }
            _ => {
                return format!("Unknown option: {}", args[i]);
            }
        }
    }
    
    if updates.is_empty() {
        return "No configuration changes specified".to_string();
    }
    
    // Apply each update
    let mut results = Vec::new();
    for (property, value) in updates {
        match ObsCommands::configure_instance(client, instance_num, property, value).await {
            Ok(result) => results.push(result.message),
            Err(e) => results.push(format!("Error updating {}: {}", property, e)),
        }
    }
    
    results.join("\n")
}

async fn handle_instance_config(args: &[&str], client: &GrpcClient) -> String {
    if args.len() < 5 {
        return "Usage: obs instance <number> set <ip|port|ssl|password> <value>".to_string();
    }
    
    let instance_num = match args[1].parse::<u32>() {
        Ok(n) => n,
        Err(_) => return "Invalid instance number".to_string(),
    };
    
    if args[2] != "set" {
        return format!("Unknown instance subcommand: {}", args[2]);
    }
    
    let property = args[3];
    let value = args[4];
    
    match ObsCommands::configure_instance(client, instance_num, property, value).await {
        Ok(result) => result.message,
        Err(e) => format!("Error configuring instance: {}", e),
    }
}

async fn handle_list_command(args: &[&str], client: &GrpcClient) -> String {
    if args.len() < 2 {
        return "Usage: obs list <scenes|sources|instances> [instance]".to_string();
    }
    
    let instance = if args.len() > 2 {
        match args[2].parse::<u32>() {
            Ok(n) => Some(n),
            Err(_) => return "Invalid instance number".to_string(),
        }
    } else {
        None
    };
    
    match args[1] {
        "instances" => {
            match ObsCommands::list_instances(client).await {
                Ok(instances) => {
                    if instances.is_empty() {
                        "No OBS instances configured".to_string()
                    } else {
                        let mut output = "OBS Instances:\n".to_string();
                        for inst in instances {
                            output.push_str(&format!(
                                "  Instance {}: {}:{} [{}] {}\n",
                                inst.instance_number,
                                inst.host,
                                inst.port,
                                if inst.use_ssl { "SSL" } else { "No SSL" },
                                if inst.is_connected { "Connected" } else { "Disconnected" }
                            ));
                        }
                        output
                    }
                }
                Err(e) => format!("Error listing instances: {}", e),
            }
        }
        "scenes" => {
            let inst = instance.unwrap_or(1);
            match ObsCommands::list_scenes(client, inst).await {
                Ok(scenes) => {
                    if scenes.is_empty() {
                        "No scenes found".to_string()
                    } else {
                        let mut output = format!("Scenes (Instance {}):\n", inst);
                        for scene in scenes {
                            output.push_str(&format!(
                                "  {}. {} {}\n",
                                scene.index + 1,
                                scene.name,
                                if scene.is_current { "[CURRENT]" } else { "" }
                            ));
                        }
                        output
                    }
                }
                Err(e) => format!("Error listing scenes: {}", e),
            }
        }
        "sources" => {
            let inst = instance.unwrap_or(1);
            match ObsCommands::list_sources(client, inst).await {
                Ok(sources) => {
                    if sources.is_empty() {
                        "No sources found".to_string()
                    } else {
                        let mut output = format!("Sources (Instance {}):\n", inst);
                        for source in sources {
                            output.push_str(&format!(
                                "  {}. {} ({}) {}\n",
                                source.index + 1,
                                source.name,
                                source.kind,
                                if source.is_visible { "" } else { "[HIDDEN]" }
                            ));
                        }
                        output
                    }
                }
                Err(e) => format!("Error listing sources: {}", e),
            }
        }
        _ => format!("Unknown list type: {}", args[1]),
    }
}

async fn handle_select_command(args: &[&str], client: &GrpcClient) -> String {
    if args.len() < 3 {
        return "Usage: obs select <scene|source> <name|number> [instance]".to_string();
    }
    
    let instance = if args.len() > 3 {
        match args[3].parse::<u32>() {
            Ok(n) => n,
            Err(_) => return "Invalid instance number".to_string(),
        }
    } else {
        1
    };
    
    match args[1] {
        "scene" => {
            match ObsCommands::select_scene(client, instance, args[2]).await {
                Ok(result) => result.message,
                Err(e) => format!("Error selecting scene: {}", e),
            }
        }
        "source" => {
            match ObsCommands::select_source(client, instance, args[2]).await {
                Ok(result) => result.message,
                Err(e) => format!("Error selecting source: {}", e),
            }
        }
        _ => format!("Unknown select type: {}", args[1]),
    }
}

async fn handle_source_command(args: &[&str], client: &GrpcClient) -> String {
    if args.len() < 2 {
        return "Usage: obs source <hide|show|refresh> [name|number] [instance]".to_string();
    }
    
    let (source_name, instance) = if args.len() == 2 {
        // No source specified, use selected source
        (None, 1)
    } else if args.len() == 3 {
        // Source specified, default instance
        (Some(args[2]), 1)
    } else {
        // Both source and instance specified
        let inst = match args[3].parse::<u32>() {
            Ok(n) => n,
            Err(_) => return "Invalid instance number".to_string(),
        };
        (Some(args[2]), inst)
    };
    
    match args[1] {
        "hide" => {
            match ObsCommands::hide_source(client, instance, source_name).await {
                Ok(result) => result.message,
                Err(e) => format!("Error hiding source: {}", e),
            }
        }
        "show" => {
            match ObsCommands::show_source(client, instance, source_name).await {
                Ok(result) => result.message,
                Err(e) => format!("Error showing source: {}", e),
            }
        }
        "refresh" => {
            match ObsCommands::refresh_source(client, instance, source_name).await {
                Ok(result) => result.message,
                Err(e) => format!("Error refreshing source: {}", e),
            }
        }
        _ => format!("Unknown source command: {}", args[1]),
    }
}

async fn handle_start_command(args: &[&str], client: &GrpcClient) -> String {
    if args.len() < 2 {
        return "Usage: obs start <stream|recording> [instance]".to_string();
    }
    
    let instance = if args.len() > 2 {
        match args[2].parse::<u32>() {
            Ok(n) => n,
            Err(_) => return "Invalid instance number".to_string(),
        }
    } else {
        1
    };
    
    match args[1] {
        "stream" => {
            match ObsCommands::start_stream(client, instance).await {
                Ok(result) => result.message,
                Err(e) => format!("Error starting stream: {}", e),
            }
        }
        "recording" => {
            match ObsCommands::start_recording(client, instance).await {
                Ok(result) => result.message,
                Err(e) => format!("Error starting recording: {}", e),
            }
        }
        _ => format!("Unknown start type: {}", args[1]),
    }
}

async fn handle_stop_command(args: &[&str], client: &GrpcClient) -> String {
    if args.len() < 2 {
        return "Usage: obs stop <stream|recording> [instance]".to_string();
    }
    
    let instance = if args.len() > 2 {
        match args[2].parse::<u32>() {
            Ok(n) => n,
            Err(_) => return "Invalid instance number".to_string(),
        }
    } else {
        1
    };
    
    match args[1] {
        "stream" => {
            match ObsCommands::stop_stream(client, instance).await {
                Ok(result) => result.message,
                Err(e) => format!("Error stopping stream: {}", e),
            }
        }
        "recording" => {
            match ObsCommands::stop_recording(client, instance).await {
                Ok(result) => result.message,
                Err(e) => format!("Error stopping recording: {}", e),
            }
        }
        _ => format!("Unknown stop type: {}", args[1]),
    }
}

async fn handle_status_command(args: &[&str], client: &GrpcClient) -> String {
    let instance = if args.len() > 1 {
        match args[1].parse::<u32>() {
            Ok(n) => n,
            Err(_) => return "Invalid instance number".to_string(),
        }
    } else {
        1
    };
    
    match ObsCommands::get_status(client, instance).await {
        Ok(status) => {
            let mut output = format!("OBS Instance {} Status:\n", instance);
            output.push_str(&format!("  Connected: {}\n", if status.is_connected { "Yes" } else { "No" }));
            if let Some(version) = status.version {
                output.push_str(&format!("  Version: {}\n", version));
            }
            if status.is_streaming {
                output.push_str(&format!("  Streaming: Yes ({}ms)\n", status.stream_time_ms.unwrap_or(0)));
            } else {
                output.push_str("  Streaming: No\n");
            }
            if status.is_recording {
                output.push_str(&format!("  Recording: Yes ({}ms)\n", status.record_time_ms.unwrap_or(0)));
            } else {
                output.push_str("  Recording: No\n");
            }
            output
        }
        Err(e) => format!("Error getting status: {}", e),
    }
}