use maowbot_common_ui::GrpcClient;
use crate::commands::obs;

/// OBS adapter for TUI - manages OBS WebSocket connections and controls
pub async fn handle_obs_command(args: &[&str], client: &GrpcClient) -> String {
    obs::handle_obs_command(args, client).await
}