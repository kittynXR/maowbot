use maowbot_common_ui::{ProcessManager, ProcessType, GrpcClient, commands::config::ConfigCommands};
use std::sync::Arc;

pub async fn handle_system_command(
    parts: &[&str],
    process_manager: &Arc<ProcessManager>,
    client: Option<&GrpcClient>,
) -> Result<String, Box<dyn std::error::Error>> {
    if parts.is_empty() {
        return Ok("Usage: system [overlay|server|shutdown] [start|stop|status]".to_string());
    }

    // Handle shutdown command
    if parts[0] == "shutdown" {
        if let Some(client) = client {
            let reason = parts.get(1).map(|s| *s);
            let grace_period = parts.get(2).and_then(|s| s.parse::<i32>().ok());
            
            match ConfigCommands::shutdown_server(client, reason, grace_period).await {
                Ok(result) => {
                    if result.accepted {
                        let shutdown_time = result.shutdown_at
                            .map(|dt| dt.format("%Y-%m-%d %H:%M:%S UTC").to_string())
                            .unwrap_or_else(|| "unknown time".to_string());
                        
                        // Also stop overlay if it's managed by this process manager
                        let _ = process_manager.stop(ProcessType::Overlay).await;
                        
                        Ok(format!("{}\nShutdown scheduled for: {}\nUI components will also be shut down.", result.message, shutdown_time))
                    } else {
                        Ok(format!("Shutdown request rejected: {}", result.message))
                    }
                }
                Err(e) => Ok(format!("Error requesting shutdown: {}", e)),
            }
        } else {
            Ok("Cannot shutdown server: not connected to gRPC service".to_string())
        }
    } else {
        let process_type = match parts[0] {
            "overlay" => ProcessType::Overlay,
            "server" => ProcessType::Server,
            _ => return Ok(format!("Unknown process type: {}. Use 'overlay', 'server', or 'shutdown'", parts[0])),
        };

    if parts.len() < 2 {
        // Just show status
        let status = process_manager.get_status(process_type).await;
        return Ok(format!(
            "{:?} status: {}, PID: {:?}",
            process_type,
            if status.running { "Running" } else { "Stopped" },
            status.pid
        ));
    }

    match parts[1] {
        "start" => {
            match process_type {
                ProcessType::Server => {
                    process_manager.ensure_server_running().await?;
                    Ok("Server started successfully".to_string())
                }
                ProcessType::Overlay => {
                    process_manager.start_overlay().await?;
                    Ok("Overlay started successfully".to_string())
                }
            }
        }
        "stop" => {
            process_manager.stop(process_type).await?;
            Ok(format!("{:?} stopped successfully", process_type))
        }
        "status" => {
            let status = process_manager.get_status(process_type).await;
            Ok(format!(
                "{:?} status: {}, PID: {:?}",
                process_type,
                if status.running { "Running" } else { "Stopped" },
                status.pid
            ))
        }
        _ => Ok(format!("Unknown command: {}. Use 'start', 'stop', or 'status'", parts[1])),
    }
    }
}