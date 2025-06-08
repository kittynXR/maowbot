use std::sync::Arc;
use maowbot_common::traits::api::BotApi;
use crate::tui_module::TuiModule;

pub async fn handle_pipeline_command(
    args: &[&str], 
    _bot_api: &Arc<dyn BotApi>,
    _tui_module: &Arc<TuiModule>,
) -> String {
    // This is a placeholder - pipeline commands should be handled by gRPC dispatcher
    format!("Pipeline command '{}' should be handled by gRPC dispatcher", args.join(" "))
}