// File: maowbot-tui/src/commands/mod.rs

use std::sync::Arc;
use maowbot_core::plugins::bot_api::BotApi;
use crate::help;
use crate::tui_module::TuiModule;

// Submodules for actual command logic:
mod account;
mod connectivity;
mod platform;
mod plugin;
mod ttv;
mod user;
mod vrchat;

// NEW:
mod member;

/// Asynchronous command dispatcher. Returns (quit_requested, optional_output_message).
pub async fn dispatch_async(
    line: &str,
    bot_api: &Arc<dyn BotApi>,
    tui_module: &Arc<TuiModule>,
) -> (bool, Option<String>) {
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.is_empty() {
        return (false, None);
    }
    let cmd = parts[0].to_lowercase();
    let args = &parts[1..];

    match cmd.as_str() {
        "help" => {
            let subcmd = args.get(0).map(|s| *s).unwrap_or("");
            let msg = help::show_command_help(subcmd);
            (false, Some(msg))
        }

        "vrchat" => {
            let msg = vrchat::handle_vrchat_command(args, bot_api).await;
            (false, Some(msg))
        }

        "list" => {
            // Example: list all plugins
            let result = bot_api.list_plugins().await;
            let mut output = String::new();
            output.push_str("All known plugins:\n");
            for p in result {
                output.push_str(&format!("  {}\n", p));
            }
            (false, Some(output))
        }

        "status" => {
            // existing logic
            let subcmd = args.get(0).map(|s| s.to_lowercase());
            let status_data = bot_api.status().await;

            let mut output = format!(
                "Uptime={}s\nConnected Plugins:\n",
                status_data.uptime_seconds
            );
            for c in status_data.connected_plugins {
                output.push_str(&format!("  {}\n", c));
            }

            output.push_str("\n--- Platforms & Accounts ---\n");
            if status_data.account_statuses.is_empty() {
                output.push_str("(No platform credentials found.)\n");
            } else {
                use std::collections::BTreeMap;
                let mut by_platform: BTreeMap<String, Vec<(String, bool)>> = BTreeMap::new();
                for acc in &status_data.account_statuses {
                    by_platform
                        .entry(acc.platform.clone())
                        .or_default()
                        .push((acc.account_name.clone(), acc.is_connected));
                }
                for (plat, accs) in by_platform {
                    output.push_str(&format!("Platform: {}\n", plat));
                    for (acct, conn) in accs {
                        let marker = if conn { "[connected]" } else { "[disconnected]" };
                        output.push_str(&format!("  - {} {}\n", marker, acct));
                    }
                }
            }

            if subcmd.as_deref() == Some("config") {
                match bot_api.list_config().await {
                    Ok(list) => {
                        output.push_str("\n--- bot_config table ---\n");
                        if list.is_empty() {
                            output.push_str("[No entries found]\n");
                        } else {
                            for (k, v) in list {
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
            let message = plugin::handle_plugin_command(args, bot_api).await;
            (false, Some(message))
        }

        // Platform config
        "platform" => {
            let message = platform::handle_platform_command(args, bot_api).await;
            (false, Some(message))
        }

        // Account
        "account" => {
            let message = account::handle_account_command(args, bot_api).await;
            (false, Some(message))
        }

        // User
        "user" => {
            let message = user::handle_user_command(args, bot_api).await;
            (false, Some(message))
        }

        // NEW: "member" command
        "member" => {
            let msg = member::handle_member_command(args, bot_api).await;
            (false, Some(msg))
        }

        // Connectivity: autostart, start, stop, chat
        "autostart" | "start" | "stop" | "chat" => {
            let message = connectivity::handle_connectivity_command(
                &[cmd.as_str()].iter().chain(args.iter()).map(|s| *s).collect::<Vec<_>>(),
                bot_api,
                tui_module
            ).await;
            (false, Some(message))
        }

        // The new TTV subcommands
        "ttv" => {
            let msg = ttv::handle_ttv_command(args, bot_api, tui_module).await;
            (false, Some(msg))
        }

        "quit" => {
            (true, Some("(TUI) shutting down...".to_string()))
        }

        _ => {
            let msg = format!("Unknown command '{}'. Type 'help' for usage.", cmd);
            (false, Some(msg))
        }
    }
}