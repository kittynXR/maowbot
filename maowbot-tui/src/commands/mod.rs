use std::sync::Arc;
use tokio::runtime::Handle;
use maowbot_core::plugins::bot_api::BotApi;

use crate::tui_module::TuiModule;

// Submodules for actual command logic:
mod account;
mod connectivity;
mod platform;
mod plugin;
mod user;
pub mod help;

// === NEW: import our new TTV module:
mod ttv;

use ttv::handle_ttv_command;

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
            let subcmd = args.get(0).map(|s| *s).unwrap_or("");
            let msg = help::show_command_help(subcmd);
            (false, Some(msg))
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
            let subcmd = args.get(0).map(|s| s.to_lowercase());
            let status_data = Handle::current().block_on(bot_api.status());

            let mut output = format!("Uptime={}s\nConnected Plugins:\n",
                                     status_data.uptime_seconds);
            for c in status_data.connected_plugins {
                output.push_str(&format!("  {}\n", c));
            }

            if subcmd.as_deref() == Some("config") {
                let config_entries = Handle::current().block_on(bot_api.list_config());
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

        // Plugin management
        "plug" => {
            let message = plugin::handle_plugin_command(args, bot_api);
            (false, Some(message))
        }

        // Platform config
        "platform" => {
            let message = platform::handle_platform_command(args, bot_api);
            (false, Some(message))
        }

        // Account
        "account" => {
            let message = account::handle_account_command(args, bot_api);
            (false, Some(message))
        }

        // User
        "user" => {
            let message = user::handle_user_command(args, bot_api);
            (false, Some(message))
        }

        // Connectivity: autostart, start, stop, chat
        "autostart" | "start" | "stop" | "chat" => {
            let message = connectivity::handle_connectivity_command(
                &[cmd.as_str()].iter().chain(args.iter()).map(|s| *s).collect::<Vec<_>>(),
                bot_api,
                tui_module
            );
            (false, Some(message))
        }

        // === NEW: TTV subcommands
        "ttv" => {
            let msg = handle_ttv_command(args, bot_api, tui_module);
            (false, Some(msg))
        }

        "quit" => {
            (true, Some("(TUI) shutting down...".to_string()))
        }

        // unrecognized
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