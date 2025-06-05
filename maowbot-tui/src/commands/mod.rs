use std::sync::Arc;
use maowbot_common::traits::api::BotApi;
use crate::tui_module::TuiModule;
use crate::help;

mod account;
mod connectivity;
mod platform;
mod plugin;
mod twitch;
mod user;
mod vrchat;
mod member;
mod command;
mod redeem;
mod osc;
mod drip;
mod config;
mod discord;
pub mod system;
pub mod test_grpc;
pub mod user_grpc;
pub mod user_adapter;
pub mod platform_adapter;
pub mod twitch_adapter;
pub mod twitch_simple_adapter;
pub mod discord_adapter;
pub mod command_adapter;
pub mod redeem_adapter;
pub mod account_adapter;
pub mod ai_adapter;
pub mod config_adapter;
pub mod plugin_adapter;
pub mod connectivity_adapter;
pub mod drip_adapter;
pub mod member_adapter;
pub mod osc_adapter;
pub mod vrchat_adapter;
pub mod credential_adapter;
pub mod connection_adapter;
pub mod unified_user_adapter;
pub mod diagnostics_adapter;
mod dispatch_grpc;
pub mod test_harness;
pub mod simulate;

pub use dispatch_grpc::dispatch_grpc;

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
            // Handled by gRPC dispatcher
            (false, Some("VRChat command should be handled by gRPC dispatcher".to_string()))
        }

        "list" => {
            let result = bot_api.list_plugins().await;
            let mut output = String::new();
            output.push_str("All known plugins:\n");
            for p in result {
                output.push_str(&format!("  {}\n", p));
            }
            (false, Some(output))
        }

        "status" => {
            let subcmd = args.get(0).map(|s| s.to_lowercase());
            let status_data = bot_api.status().await;

            let mut output = format!("Uptime={}s\nConnected Plugins:\n", status_data.uptime_seconds);
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

        "osc" => {
            // Handled by gRPC dispatcher
            (false, Some("OSC command should be handled by gRPC dispatcher".to_string()))
        }

        "drip" => {
            let output = drip::handle_drip_command(args, bot_api, tui_module).await;
            (false, Some(output))
        }

        "plugin" => {
            let message = plugin::handle_plugin_command(args, bot_api).await;
            (false, Some(message))
        }

        "platform" => {
            let message = platform::handle_platform_command(args, bot_api).await;
            (false, Some(message))
        }

        "account" => {
            let message = account::handle_account_command(args, bot_api).await;
            (false, Some(message))
        }

        "user" => {
            let message = user::handle_user_command(args, bot_api).await;
            (false, Some(message))
        }

        "member" => {
            let msg = member::handle_member_command(args, bot_api).await;
            (false, Some(msg))
        }

        "command" => {
            let msg = command::handle_command_command(args, bot_api).await;
            (false, Some(msg))
        }

        "redeem" => {
            let msg = redeem::handle_redeem_command(args, bot_api).await;
            (false, Some(msg))
        }

        "connection" => {
            let full_args = [cmd.as_str()].iter().chain(args.iter()).map(|s| *s).collect::<Vec<_>>();
            let message = connectivity::handle_connectivity_command(&full_args[1..], bot_api, tui_module).await;
            (false, Some(message))
        }
        
        // Keep legacy support for now
        "autostart" | "start" | "stop" | "chat" => {
            let message = connectivity::handle_connectivity_command(
                &[cmd.as_str()].iter().chain(args.iter()).map(|s| *s).collect::<Vec<_>>(),
                bot_api,
                tui_module
            ).await;
            (false, Some(message))
        }

        "twitch" => {
            let msg = twitch::handle_twitch_command(args, bot_api, tui_module).await;
            (false, Some(msg))
        }

        // NEW:
        "discord" => {
            let msg = discord::handle_discord_command(args, bot_api).await;
            (false, Some(msg))
        }

        "ai" => {
            // Handled by gRPC dispatcher
            (false, Some("AI command should be handled by gRPC dispatcher".to_string()))
        }

        "config" => {
            let msg = config::handle_config_command(args, bot_api).await;
            (false, Some(msg))
        }

        "test_grpc" => {
            let msg = test_grpc::handle_test_grpc_command(args).await;
            (false, Some(msg))
        }

        "test_harness" => {
            match test_harness::TestHarnessCommand::execute_from_args(args).await {
                Ok(_) => (false, Some("Test harness completed successfully".to_string())),
                Err(e) => (false, Some(format!("Test harness failed: {}", e))),
            }
        }

        "simulate" => {
            let msg = simulate::handle_simulate_command(args, bot_api, tui_module).await;
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
