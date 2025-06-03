use std::sync::Arc;
use maowbot_common_ui::GrpcClient;
use crate::help;
use crate::TuiModule;

// Import the new adapters
use super::user_adapter;
use super::platform_adapter;
use super::ttv_adapter;
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

pub async fn dispatch_grpc(
    line: &str,
    client: &GrpcClient,
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

        "user" => {
            let message = user_adapter::handle_user_command(args, client).await;
            (false, Some(message))
        }

        "platform" => {
            let message = platform_adapter::handle_platform_command(args, client).await;
            (false, Some(message))
        }

        "ttv" => {
            let msg = ttv_adapter::handle_ttv_command(args, client, tui_module).await;
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

        "ai" => {
            let msg = ai_adapter::handle_ai_command(args, client).await;
            (false, Some(msg))
        }

        "config" => {
            let msg = config_adapter::handle_config_command(args, client).await;
            (false, Some(msg))
        }

        "plug" => {
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

        "autostart" | "start" | "stop" | "chat" => {
            let msg = connectivity_adapter::handle_connectivity_command(
                &[cmd.as_str()].iter().chain(args.iter()).map(|s| *s).collect::<Vec<_>>(),
                client,
                tui_module
            ).await;
            (false, Some(msg))
        }

        "drip" => {
            let msg = drip_adapter::handle_drip_command(args, client, tui_module).await;
            (false, Some(msg))
        }

        "member" => {
            let msg = member_adapter::handle_member_command(args, client).await;
            (false, Some(msg))
        }

        "osc" => {
            let msg = osc_adapter::handle_osc_command(args, client, tui_module).await;
            (false, Some(msg))
        }

        "vrchat" => {
            let msg = vrchat_adapter::handle_vrchat_command(args, client).await;
            (false, Some(msg))
        }

        "test_grpc" => {
            let msg = test_grpc::handle_test_grpc_command(args).await;
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