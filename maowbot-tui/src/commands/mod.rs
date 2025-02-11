// File: maowbot-tui/src/commands/mod.rs

use std::sync::Arc;
use maowbot_core::plugins::bot_api::BotApi;
use tokio::runtime::Handle;

mod account;
mod platform;
mod plugin;
mod user;
pub mod connectivity;

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
  autostart <on/off> <platform> <account>
  start/stop <platform> <account>
  chat <on/off> [platform] [account]
  quit
";
            (false, Some(help.to_string()))
        }
        "list" => {
            let result = Handle::current().block_on(bot_api.list_plugins());
            let mut output = String::new();
            output.push_str("All known plugins:\n");
            for p in result {
                output.push_str(&format!("  {}\n", p));
            }
            (false, Some(output))
        }
        "status" => {
            // Now we actually block_on the final result
            let status_data = Handle::current().block_on(bot_api.status());
            // `status_data` is a `StatusData` struct, so we can read fields
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
        "autostart" | "start" | "stop" | "chat" => {
            let message = connectivity::handle_connectivity_command(
                &[cmd.as_str()].iter().chain(args.iter()).map(|s| *s).collect::<Vec<_>>(),
                bot_api
            );
            (false, Some(message))
        }
        "quit" => {
            (true, Some("(TUI) shutting down...".to_string()))
        },
        _ => {
            if cmd.is_empty() {
                (false, None)
            } else {
                let msg = format!("Unknown command '{}'. Type 'help' for usage.", cmd);
                (false, Some(msg))
            }
        }
    }
}
