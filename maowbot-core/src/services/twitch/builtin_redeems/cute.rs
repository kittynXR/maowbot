// File: maowbot-core/src/services/twitch/builtin_redeems/cute.rs

use tracing::info;
use crate::Error;
use crate::services::twitch::redeem_service::RedeemHandlerContext;
use crate::platforms::twitch::requests::channel_points::CustomRewardBody;
use crate::platforms::twitch::requests::channel_points::Redemption;

/// A small demonstration redeem that does not require user input.
/// We reject (“cancel”) it immediately, thus returning points to the user.
/// The cost is 50 channel points, stored in the DB. If triggered from Twitch:
///   - We update the redemption status to “CANCELED” via Helix,
///   - Then log usage in the DB via `redeem_service`.
///
/// For real usage, you might do something more interesting here.
pub async fn handle_cute_redeem(
    ctx: &RedeemHandlerContext<'_>,
    redemption: &Redemption,
) -> Result<(), Error> {
    info!("Builtin 'cute' redeem triggered for user_id={} reward='{}'",
          redemption.user_id, redemption.reward.title);

    // 1) Update the redemption status to CANCELED (which gives points back).
    if let Some(client) = &ctx.helix_client {
        let broadcaster_id = &redemption.broadcaster_id;
        let reward_id = &redemption.reward.id;
        let redemption_id = &redemption.id;
        // Cancel is done by "update_redemption_status" with status="CANCELED"
        let _ = client.update_redemption_status(
            broadcaster_id,
            reward_id,
            &[ redemption_id ],
            "CANCELED"  // or "CANCELED" => user gets points back
        ).await?;
    }

    // 2) Optionally, you could also do some minimal “update reward” logic, e.g. changing cost dynamically.
    // For demonstration, we leave it alone. If you wanted to tweak cost:
    //   let body = CustomRewardBody { cost: Some(55), ..Default::default() };
    //   let _ = client.update_custom_reward(broadcaster_id, reward_id, &body).await?;

    // 3) That’s it. The main redeem_service handle_redeem call will record usage in DB.
    Ok(())
}
