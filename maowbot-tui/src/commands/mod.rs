use std::sync::Arc;
use maowbot_core::plugins::bot_api::BotApi;

mod account;
mod platform;
mod plugin;
mod user;

use crate::tui_module::tui_block_on;

pub fn dispatch(
    line: &str,
    bot_api: &Arc<dyn BotApi>,
) -> (bool, Option<String>) {

    let parts: Vec<&str> = line.split_whitespace().collect();
    let cmd = parts.get(0).unwrap_or(&"").to_lowercase();
    let args = &parts[1..];

    match cmd.as_str() {
        "help" => {
            let help = "\
Commands:
  help
  list
  status
  plug   <enable|disable|remove> <name>
  platform <add|remove|list|show> ...
  account  <add|remove|list|show> [platform] [username]
  user     <add|remove|edit|info|search> ...
  quit
";
            (false, Some(help.to_string()))
        }
        "list" => {
            let result = tui_block_on(bot_api.list_plugins());
            let mut output = String::new();
            output.push_str("All known plugins:\n");
            for p in result {
                output.push_str(&format!("  {}\n", p));
            }
            (false, Some(output))
        }
        "status" => {
            let status_data = tui_block_on(bot_api.status());
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
        "platform" => {
            let message = platform::handle_platform_command(args, bot_api);
            (false, Some(message))
        }
        "account" => {
            let message = account::handle_account_command(args, bot_api);
            (false, Some(message))
        }
        "user" => {
            let message = user::handle_user_command(args, bot_api);
            (false, Some(message))
        }
        "quit" => {
            // We'll shut down in TuiModule after we return (quit_requested = true).
            (true, Some("(TUI) shutting down...".to_string()))
        },
        _ => {
            if cmd.is_empty() {
                (false, None) // ignore blank lines
            } else {
                let msg = format!("Unknown command '{}'. Type 'help' for usage.", cmd);
                (false, Some(msg))
            }
        }
    }
}