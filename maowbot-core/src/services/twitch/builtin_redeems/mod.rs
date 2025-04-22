// File: maowbot-core/src/services/twitch/builtin_redeems/mod.rs

pub mod cute;
pub mod pillo;
pub mod osc_triggers;
mod askai;

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
        "cat_trap" => {
            osc_triggers::handle_cattrap_redeem(ctx, redemption).await?;
        },
        "pillo" => {
            osc_triggers::handle_pillo_redeem(ctx, redemption).await?;
        },
        "askai" => {
            askai::handle_askai_redeem(ctx, redemption).await?;
        },
        "askmao" => {
            askai::handle_askmao_redeem(ctx, redemption).await?;
        },
        "askai_search" => {
            askai::handle_askai_search_redemption(ctx, redemption).await?;
        }
        _ => {
            info!("No built-in redeem logic found for command_name='{}'", command_name);
        }
    }
    Ok(())
}
