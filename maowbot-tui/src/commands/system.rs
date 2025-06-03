use maowbot_common_ui::{ProcessManager, ProcessType};
use std::sync::Arc;

pub async fn handle_system_command(
    parts: &[&str],
    process_manager: &Arc<ProcessManager>,
) -> Result<String, Box<dyn std::error::Error>> {
    if parts.is_empty() {
        return Ok("Usage: system [overlay|server] [start|stop|status]".to_string());
    }

    let process_type = match parts[0] {
        "overlay" => ProcessType::Overlay,
        "server" => ProcessType::Server,
        _ => return Ok(format!("Unknown process type: {}. Use 'overlay' or 'server'", parts[0])),
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