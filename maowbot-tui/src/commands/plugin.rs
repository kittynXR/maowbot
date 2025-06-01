// File: maowbot-tui/src/commands/plugin.rs
use std::sync::Arc;
use maowbot_common::traits::api::BotApi;

/// Asynchronously handle "plug <enable|disable|remove> <pluginName>"
pub async fn handle_plugin_command(args: &[&str], bot_api: &Arc<dyn BotApi>) -> String {
    if args.len() < 2 {
        return "Usage: plug <enable|disable|remove> <pluginName>".to_string();
    }
    let subcmd = args[0];
    let plugin_name = args[1];

    match subcmd {
        "enable" => {
            match bot_api.toggle_plugin(plugin_name, true).await {
                Ok(_) => format!("Plugin '{}' is now ENABLED", plugin_name),
                Err(e) => format!("Error enabling plugin '{}': {:?}", plugin_name, e),
            }
        }
        "disable" => {
            match bot_api.toggle_plugin(plugin_name, false).await {
                Ok(_) => format!("Plugin '{}' is now DISABLED", plugin_name),
                Err(e) => format!("Error disabling plugin '{}': {:?}", plugin_name, e),
            }
        }
        "remove" => {
            match bot_api.remove_plugin(plugin_name).await {
                Ok(_) => format!("Plugin '{}' removed.", plugin_name),
                Err(e) => format!("Error removing '{}': {:?}", plugin_name, e),
            }
        }
        _ => "Usage: plug <enable|disable|remove> <pluginName>".to_string(),
    }
}