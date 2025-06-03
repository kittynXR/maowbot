use std::sync::Arc;
use maowbot_common_ui::GrpcClient;
use crate::tui_module::TuiModule;
use crate::help;

// Import the new adapters
use super::user_adapter;
use super::platform_adapter;
use super::ttv_adapter;
use super::test_grpc;

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

        "test_grpc" => {
            let msg = test_grpc::handle_test_grpc_command(args).await;
            (false, Some(msg))
        }

        "quit" => {
            (true, Some("(TUI) shutting down...".to_string()))
        }

        _ => {
            let msg = format!("Unknown command '{}'. Type 'help' for usage.\n(Note: Not all commands have been converted to gRPC yet)", cmd);
            (false, Some(msg))
        }
    }
}