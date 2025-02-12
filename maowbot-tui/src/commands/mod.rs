use std::sync::Arc;
use tokio::runtime::Handle;
use maowbot_core::plugins::bot_api::BotApi;

use crate::tui_module::TuiModule;

mod account;
mod connectivity;
mod platform;
mod plugin;
mod user;

/// We pass `&TuiModule` so that certain commands (like “chat on/off”)
/// can update the shared chat state.
pub fn dispatch(
    line: &str,
    bot_api: &Arc<dyn BotApi>,
    tui_module: &Arc<TuiModule>,
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
                  status <config>
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
            let result = tokio::runtime::Handle::current().block_on(bot_api.list_plugins());
            let mut output = String::new();
            output.push_str("All known plugins:\n");
            for p in result {
                output.push_str(&format!("  {}\n", p));
            }
            (false, Some(output))
        }

        "status" => {
            let subcmd = args.get(0).map(|s| s.to_lowercase());
            // 1) Always show normal status
            let status_data = tokio::runtime::Handle::current().block_on(bot_api.status());

            let mut output = format!("Uptime={}s\nConnected Plugins:\n",
                                     status_data.uptime_seconds);
            for c in status_data.connected_plugins {
                output.push_str(&format!("  {}\n", c));
            }

            // 2) If the user typed "status config", also list all config
            if subcmd.as_deref() == Some("config") {
                let config_entries = tokio::runtime::Handle::current().block_on(bot_api.list_config());
                match config_entries {
                    Ok(list) => {
                        output.push_str("\n--- bot_config table ---\n");
                        if list.is_empty() {
                            output.push_str("[No entries found]\n");
                        } else {
                            for (k,v) in list {
                                output.push_str(&format!("  {} = {}\n", k, v));
                            }
                        }
                    }
                    Err(e) => {
                        output.push_str(&format!("\n[Error listing bot_config => {:?}]\n", e));
                    }
                }
            }

            (false, Some(output))
        }


        "plug" => {
            // same as before
            let message = plugin::handle_plugin_command(args, bot_api);
            (false, Some(message))
        }

        "platform" => {
            // same as before
            let message = platform::handle_platform_command(args, bot_api);
            (false, Some(message))
        }

        "account" => {
            // same as before
            let message = account::handle_account_command(args, bot_api);
            (false, Some(message))
        }

        "user" => {
            // same as before
            let message = user::handle_user_command(args, bot_api);
            (false, Some(message))
        }

        // Connect commands
        "autostart" | "start" | "stop" | "chat" => {
            let message = connectivity::handle_connectivity_command(
                &[cmd.as_str()].iter().chain(args.iter()).map(|s| *s).collect::<Vec<_>>(),
                bot_api,
                tui_module
            );
            (false, Some(message))
        }

        "quit" => {
            (true, Some("(TUI) shutting down...".to_string()))
        }

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
