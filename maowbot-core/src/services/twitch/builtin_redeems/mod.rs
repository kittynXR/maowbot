// File: maowbot-core/src/services/twitch/builtin_redeems/mod.rs

pub mod cute;

// Re-export or define a small “dispatcher” function:
use tracing::info;
use crate::Error;
use crate::platforms::twitch::requests::channel_points::Redemption;
use crate::services::twitch::redeem_service::RedeemHandlerContext;

/// If plugin_name=="builtin", we look at the `command_name` column
/// in the `redeems` table and dispatch accordingly.
pub async fn handle_builtin_redeem(
    ctx: &RedeemHandlerContext<'_>,
    redemption: &Redemption,
    command_name: &str,
) -> Result<(), Error> {
    match command_name.to_lowercase().as_str() {
        "cute" => {
            cute::handle_cute_redeem(ctx, redemption).await?;
        },
        // Additional built-in redeems can be matched here...
        _ => {
            info!("No built-in redeem logic found for command_name='{}'", command_name);
        }
    }
    Ok(())
}
