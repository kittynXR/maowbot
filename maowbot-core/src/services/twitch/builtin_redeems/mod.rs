// File: maowbot-core/src/services/twitch/builtin_redeems/mod.rs

pub mod cute;
pub mod osc_triggers;
pub mod askai;

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
    // First check if this redeem has an OSC trigger configured
    // Get the redeem by reward_id to get its UUID
    if let Some(redeem) = &ctx.redeem_repo.get_redeem_by_reward_id("twitch-eventsub", &redemption.reward.id).await? {
        // Check if there's an OSC trigger for this redeem
        let has_trigger = if let Some(plugin_manager) = ctx.redeem_service.platform_manager.plugin_manager() {
            if let Some(osc_toggle_repo) = &plugin_manager.osc_toggle_repo {
                matches!(osc_toggle_repo.get_trigger_by_redeem_id(redeem.redeem_id).await, Ok(Some(_)))
            } else {
                false
            }
        } else {
            false
        };
        
        if has_trigger {
            info!("Found OSC trigger for redeem {}, using generic handler", redeem.redeem_id);
            return osc_triggers::handle_generic_osc_toggle(ctx, redemption, redeem.redeem_id).await;
        }
    }
    
    // Fall back to hardcoded handlers
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
