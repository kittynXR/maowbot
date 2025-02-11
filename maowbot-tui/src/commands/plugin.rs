// File: maowbot-tui/src/commands/plugin.rs

use std::sync::Arc;
use maowbot_core::plugins::bot_api::BotApi;
use maowbot_core::Error;
use tokio::runtime::Handle;

pub fn handle_plugin_command(args: &[&str], bot_api: &Arc<dyn BotApi>) -> String {
    if args.len() < 2 {
        return "Usage: plug <enable|disable|remove> <pluginName>".to_string();
    }
    let subcmd = args[0];
    let plugin_name = args[1];

    match subcmd {
        "enable" | "disable" => {
            let enable = subcmd == "enable";
            let result = Handle::current().block_on(async {
                bot_api.toggle_plugin(plugin_name, enable).await
            });
            match result {
                Ok(_) => format!(
                    "Plugin '{}' is now {}",
                    plugin_name,
                    if enable { "ENABLED" } else { "DISABLED" }
                ),
                Err(e) => format!("Error toggling plugin '{}': {:?}", plugin_name, e),
            }
        }
        "remove" => {
            let result = Handle::current().block_on(async {
                bot_api.remove_plugin(plugin_name).await
            });
            match result {
                Ok(_) => format!("Plugin '{}' removed.", plugin_name),
                Err(e) => format!("Error removing '{}': {:?}", plugin_name, e),
            }
        }
        _ => "Usage: plug <enable|disable|remove> <pluginName>".to_string(),
    }
}
