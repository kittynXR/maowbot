use crate::Error;
use crate::services::twitch::redeem_service::RedeemService;
use crate::services::user_service::UserService;
use crate::platforms::twitch_eventsub::events::channel_points::ChannelPointsCustomRewardRedemption;
use crate::platforms::twitch::requests::channel_points::{Redemption, RedemptionReward};

/// For "channel.channel_points_custom_reward_redemption.add"
pub async fn handle_custom_reward_redemption_add(
    evt: ChannelPointsCustomRewardRedemption,
    redeem_service: &RedeemService,
    user_service: &UserService,
) -> Result<(), Error> {
    // 1) Convert the incoming `ChannelPointsCustomRewardRedemption` to the Helix-like `Redemption` structure
    let redemption = Redemption {
        broadcaster_id: evt.broadcaster_user_id.clone(),
        broadcaster_login: Some(evt.broadcaster_user_login.clone()),
        broadcaster_name: Some(evt.broadcaster_user_name.clone()),
        id: evt.id.clone(),
        user_id: evt.user_id.clone(),     // note: still a string
        user_name: Some(evt.user_name.clone()),
        user_login: Some(evt.user_login.clone()),
        user_input: evt.user_input.clone(),
        status: evt.status.clone(),
        redeemed_at: evt.redeemed_at.to_rfc3339(), // keep the same timestamp
        reward: RedemptionReward {
            id: evt.reward.id.clone(),
            title: evt.reward.title.clone(),
            prompt: evt.reward.prompt.clone(),
            cost: evt.reward.cost as u64,
        },
    };

    // 2) Convert the event's user_id to your internal DB user. This uses the user_service to unify identity.
    //    Use "twitch-irc" as the platform for consistent user lookup
    //    The platform_user_id is the numeric string from the event. The user_name is from the event as well.
    let user = user_service
        .get_or_create_user("twitch-eventsub", &evt.user_id, Some(&evt.user_name))
        .await?;

    // 3) Call your RedeemService logic, using "twitch-irc" for platform consistency
    redeem_service
        .handle_incoming_redeem(
            "twitch-eventsub",             // Use consistent platform name
            &evt.reward.id,            // reward_id
            user.user_id,              // Uuid from DB
            &evt.broadcaster_user_name,// channel/broadcaster context
            &redemption,
        )
        .await?;

    Ok(())
}

// ---------------------------------------------------------------------------
// For completeness, these placeholders show how you'd handle other points events:
// ---------------------------------------------------------------------------
pub async fn handle_automatic_reward_redemption() -> Result<(), Error> {
    Ok(())
}

pub async fn handle_custom_reward_add() -> Result<(), Error> {
    Ok(())
}

pub async fn handle_custom_reward_update() -> Result<(), Error> {
    Ok(())
}

pub async fn handle_custom_reward_remove() -> Result<(), Error> {
    Ok(())
}

pub async fn handle_custom_reward_redemption_update() -> Result<(), Error> {
    Ok(())
}
