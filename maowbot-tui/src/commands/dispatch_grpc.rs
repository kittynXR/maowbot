use std::sync::Arc;
use maowbot_common_ui::{GrpcClient, ProcessManager};
use crate::help;
use crate::tui_module_simple::SimpleTuiModule;

// Import the new adapters
use super::user_adapter;
use super::platform_adapter;
use super::twitch_adapter;
use super::test_grpc;
use super::command_adapter;
use super::discord_adapter;
use super::redeem_adapter;
use super::account_adapter;
use super::ai_adapter;
use super::config_adapter;
use super::plugin_adapter;
use super::connectivity_adapter;
use super::drip_adapter;
use super::member_adapter;
use super::osc_adapter;
use super::vrchat_adapter;
use super::obs_adapter;
use super::credential_adapter;
use super::connection_adapter;
use super::unified_user_adapter;
use super::diagnostics_adapter;
use super::system;

pub async fn dispatch_grpc(
    line: &str,
    client: &GrpcClient,
    tui_module: &Arc<SimpleTuiModule>,
    process_manager: &Arc<ProcessManager>,
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

        "user" => {
            let message = unified_user_adapter::handle_user_command(args, client).await;
            (false, Some(message))
        }

        "platform" => {
            let message = platform_adapter::handle_platform_command(args, client).await;
            (false, Some(message))
        }

        "twitch" => {
            let msg = twitch_adapter::handle_twitch_command(args, client, tui_module).await;
            (false, Some(msg))
        }

        "command" => {
            let msg = command_adapter::handle_command_command(args, client).await;
            (false, Some(msg))
        }

        "discord" => {
            let msg = discord_adapter::handle_discord_command(args, client).await;
            (false, Some(msg))
        }

        "redeem" => {
            let msg = redeem_adapter::handle_redeem_command(args, client).await;
            (false, Some(msg))
        }

        "account" => {
            let msg = account_adapter::handle_account_command(args, client).await;
            (false, Some(msg))
        }
        
        "credential" => {
            let msg = credential_adapter::handle_credential_command(args, client).await;
            (false, Some(msg))
        }

        "ai" => {
            let msg = ai_adapter::AiAdapter::handle_command(args, client).await;
            (false, Some(msg))
        }

        "config" => {
            let msg = config_adapter::handle_config_command(args, client).await;
            (false, Some(msg))
        }

        "plugin" => {
            let msg = plugin_adapter::handle_plugin_command(args, client).await;
            (false, Some(msg))
        }

        "list" => {
            let msg = plugin_adapter::handle_list_command(client).await;
            (false, Some(msg))
        }

        "status" => {
            let msg = plugin_adapter::handle_status_command(args, client).await;
            (false, Some(msg))
        }

        "connection" => {
            let msg = connection_adapter::handle_connection_command(args, client, tui_module).await;
            (false, Some(msg))
        }
        
        // Legacy command redirects
        "autostart" | "start" | "stop" | "chat" => {
            (false, Some(format!("The '{}' command has been merged into 'connection'.\nUse 'connection {}' instead.", cmd, cmd)))
        }

        "drip" => {
            let msg = drip_adapter::handle_drip_command(args, client, tui_module).await;
            (false, Some(msg))
        }

        "member" => {
            (false, Some("The 'member' command has been merged into 'user'.\nUse 'user' for all user management functionality.".to_string()))
        }

        "osc" => {
            let msg = osc_adapter::handle_osc_command(args, client, tui_module).await;
            (false, Some(msg))
        }

        "vrchat" => {
            let msg = vrchat_adapter::handle_vrchat_command(args, client).await;
            (false, Some(msg))
        }
        
        "obs" => {
            let msg = obs_adapter::handle_obs_command(args, client).await;
            (false, Some(msg))
        }

        "test_grpc" => {
            let msg = test_grpc::handle_test_grpc_command(args).await;
            (false, Some(msg))
        }

        "system" => {
            match system::handle_system_command(args, process_manager, Some(client)).await {
                Ok(msg) => {
                    // Check if this was a shutdown command
                    let should_quit = args.get(0) == Some(&"shutdown") && msg.contains("Shutdown scheduled");
                    (should_quit, Some(msg))
                },
                Err(e) => (false, Some(format!("System command error: {}", e))),
            }
        }
        
        "diagnostics" | "diag" => {
            let msg = diagnostics_adapter::handle_diagnostics_command(args, client).await;
            (false, Some(msg))
        }

        "quit" => {
            (true, Some("(TUI) shutting down...".to_string()))
        }

        // Renamed command redirects
        "ttv" => {
            (false, Some("The 'ttv' command has been renamed to 'twitch'.\nUse 'twitch' instead.".to_string()))
        }
        
        "plug" => {
            (false, Some("The 'plug' command has been renamed to 'plugin'.\nUse 'plugin' instead.".to_string()))
        }

        _ => {
            let msg = format!("Unknown command '{}'. Type 'help' for usage.", cmd);
            (false, Some(msg))
        }
    }
}