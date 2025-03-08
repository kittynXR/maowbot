use crate::Error;

/// Stubs for channel_points related events:
///  - channel.channel_points_automatic_reward_redemption.add
///  - channel.channel_points_custom_reward.add
///  - channel.channel_points_custom_reward.update
///  - channel.channel_points_custom_reward.remove
///  - channel.channel_points_custom_reward_redemption.add
///  - channel.channel_points_custom_reward_redemption.update
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

pub async fn handle_custom_reward_redemption_add() -> Result<(), Error> {
    Ok(())
}

pub async fn handle_custom_reward_redemption_update() -> Result<(), Error> {
    Ok(())
}
