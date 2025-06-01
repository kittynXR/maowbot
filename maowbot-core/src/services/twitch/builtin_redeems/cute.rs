use tracing::info;
use crate::Error;
use crate::services::twitch::redeem_service::RedeemHandlerContext;
use crate::platforms::twitch::requests::channel_points::Redemption;

/// A small demonstration redeem that cancels itself immediately, returning points to the user.
/// For real usage, you might do something more interesting here.
pub async fn handle_cute_redeem(
    ctx: &RedeemHandlerContext<'_>,
    redemption: &Redemption,
) -> Result<(), Error> {
    info!(
        "Builtin 'cute' redeem triggered for user_id={} reward='{}'",
        redemption.user_id, redemption.reward.title
    );

    // Example: Update the redemption status to CANCELED (return points to user).
    if let Some(client) = &ctx.helix_client {
        let broadcaster_id = &redemption.broadcaster_id;
        let reward_id = &redemption.reward.id;
        let redemption_id = &redemption.id;

        // Cancel by setting status = "CANCELED"
        let _ = client
            .update_redemption_status(
                broadcaster_id,
                reward_id,
                &[redemption_id],
                "CANCELED",
            )
            .await?;
    }

    Ok(())
}