// File: maowbot-core/src/services/twitch/builtin_redeems/mod.rs

pub mod cute;

// Re-export or define a small “dispatcher” function:
use crate::Error;
use crate::platforms::twitch::requests::channel_points::Redemption;
use crate::services::twitch::redeem_service::RedeemHandlerContext;

/// If you expect multiple built-in redeems, you can do a match by “internal name,”
/// or by the reward title. In this example, we do a simple function that
/// checks if the reward_name is “cute”, then calls `handle_cute_redeem`.
pub async fn handle_builtin_redeem(
    ctx: &RedeemHandlerContext<'_>,
    redemption: &Redemption,
    reward_name: &str,
) -> Result<(), Error> {
    match reward_name.to_lowercase().as_str() {
        "cute" => {
            super::builtin_redeems::cute::handle_cute_redeem(ctx, redemption).await?;
        },
        // Add further matches for your other future built-in redeems:
        _ => {}
    }
    Ok(())
}
