// maowbot-tui/src/commands/mod.rs

use std::sync::Arc;
use maowbot_core::plugins::bot_api::BotApi;

mod auth;
mod plugin;

pub fn dispatch(
    line: &str,
    bot_api: &Arc<dyn BotApi>,
) -> (bool, Option<String>) {
    // Return (quit_requested, output_string)

    // Split by whitespace
    let parts: Vec<&str> = line.split_whitespace().collect();
    let cmd = parts[0].to_lowercase();
    let args = &parts[1..];

    match cmd.as_str() {
        "help" => {
            let help = "\
Commands:
  help
  list
  status
  plug <enable|disable|remove> <name>
  auth <add|remove|list> [...]
  quit
";
            (false, Some(help.to_string()))
        }
        "list" => {
            // e.g. plugin listing
            let result = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(bot_api.list_plugins());
            let mut output = String::new();
            output.push_str("All known plugins:\n");
            for p in result {
                output.push_str(&format!("  {}\n", p));
            }
            (false, Some(output))
        }
        "status" => {
            let status_data = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(bot_api.status());
            let mut output = format!("Uptime={}s\nConnected Plugins:\n",
                                     status_data.uptime_seconds);
            for c in status_data.connected_plugins {
                output.push_str(&format!("  {}\n", c));
            }
            (false, Some(output))
        }
        "plug" => {
            let message = plugin::handle_plugin_command(args, bot_api);
            (false, Some(message))
        }
        "auth" => {
            let message = auth::handle_auth_command(args, bot_api);
            (false, Some(message))
        }
        "quit" => {
            // user wants to shut down
            (true, Some("(TUI) shutting down...".to_string()))
        }
        _ => {
            let msg = format!("Unknown command '{}'. Type 'help' for usage.", cmd);
            (false, Some(msg))
        }
    }
}
