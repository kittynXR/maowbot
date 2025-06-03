// Plugin command adapter for TUI
use maowbot_common_ui::{GrpcClient, commands::plugin::{PluginCommands, PluginState}};

pub async fn handle_plugin_command(args: &[&str], client: &GrpcClient) -> String {
    if args.len() < 2 {
        return "Usage: plug <enable|disable|remove> <pluginName>".to_string();
    }
    let subcmd = args[0];
    let plugin_name = args[1];

    match subcmd {
        "enable" => {
            match PluginCommands::enable_plugin(client, plugin_name).await {
                Ok(result) => {
                    let state_str = match result.state {
                        PluginState::Running => "ENABLED",
                        PluginState::Loaded => "LOADED",
                        PluginState::Error => "ERROR",
                        _ => "ENABLED",
                    };
                    format!("Plugin '{}' is now {}", plugin_name, state_str)
                }
                Err(e) => format!("Error enabling plugin '{}': {}", plugin_name, e),
            }
        }
        "disable" => {
            match PluginCommands::disable_plugin(client, plugin_name).await {
                Ok(_) => format!("Plugin '{}' is now DISABLED", plugin_name),
                Err(e) => format!("Error disabling plugin '{}': {}", plugin_name, e),
            }
        }
        "remove" => {
            match PluginCommands::remove_plugin(client, plugin_name).await {
                Ok(_) => format!("Plugin '{}' removed.", plugin_name),
                Err(e) => format!("Error removing '{}': {}", plugin_name, e),
            }
        }
        _ => "Usage: plug <enable|disable|remove> <pluginName>".to_string(),
    }
}

/// Handle the list command to show all plugins
pub async fn handle_list_command(client: &GrpcClient) -> String {
    match PluginCommands::list_plugins(client, false).await {
        Ok(result) => {
            let mut output = String::new();
            output.push_str("All known plugins:\n");
            for plugin in result.plugins {
                output.push_str(&format!("  {}\n", plugin.name));
            }
            output
        }
        Err(e) => format!("Error listing plugins: {}", e),
    }
}

/// Handle the status command
pub async fn handle_status_command(args: &[&str], client: &GrpcClient) -> String {
    match PluginCommands::get_system_status(client).await {
        Ok(status) => {
            let mut output = format!(
                "Uptime={}s\nConnected Plugins:\n", 
                status.uptime_seconds
            );
            
            for plugin in status.connected_plugins {
                output.push_str(&format!("  {}\n", plugin));
            }
            
            // If "config" subcommand is provided, also show config
            let subcmd = args.get(0).map(|s| s.to_lowercase());
            if subcmd.as_deref() == Some("config") {
                // Import config commands
                use maowbot_common_ui::commands::config::ConfigCommands;
                
                match ConfigCommands::list_configs(client).await {
                    Ok(list) => {
                        output.push_str("\n--- bot_config table ---\n");
                        if list.configs.is_empty() {
                            output.push_str("[No entries found]\n");
                        } else {
                            for config in list.configs {
                                output.push_str(&format!("  {} = {}\n", config.key, config.value));
                            }
                        }
                    }
                    Err(e) => {
                        output.push_str(&format!("\n[Error listing bot_config => {}]\n", e));
                    }
                }
            }
            
            output
        }
        Err(e) => format!("Error getting system status: {}", e),
    }
}