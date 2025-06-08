use async_trait::async_trait;
use tracing::{debug, info, error};
use maowbot_common::models::platform::Platform;

use crate::Error;
use crate::eventbus::{BotEvent, TwitchEventSubData};
use crate::platforms::twitch_eventsub::events::ChannelPointsCustomRewardRedemption;
use crate::platforms::twitch::requests::channel_points::{Redemption, RedemptionReward};
use crate::services::event_context::EventContext;
use crate::services::event_handler::{EventHandler, TypedEventHandler};

/// Handler for Twitch channel points redemption events
pub struct ChannelPointsRedemptionHandler;

impl ChannelPointsRedemptionHandler {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl EventHandler for ChannelPointsRedemptionHandler {
    fn id(&self) -> &str {
        "twitch.channel_points.redemption"
    }

    fn event_types(&self) -> Vec<String> {
        vec!["channel.channel_points_custom_reward_redemption.add".to_string()]
    }

    fn platforms(&self) -> Vec<Platform> {
        vec![Platform::Twitch, Platform::TwitchEventSub]
    }

    async fn handle(&self, event: &BotEvent, ctx: &EventContext) -> Result<bool, Error> {
        match event {
            BotEvent::TwitchEventSub(TwitchEventSubData::ChannelPointsCustomRewardRedemptionAdd(evt)) => {
                self.handle_typed(evt, ctx).await?;
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    fn priority(&self) -> i32 {
        100 // Normal priority
    }
}

#[async_trait]
impl TypedEventHandler<ChannelPointsCustomRewardRedemption> for ChannelPointsRedemptionHandler {
    async fn handle_typed(&self, evt: &ChannelPointsCustomRewardRedemption, ctx: &EventContext) -> Result<(), Error> {
        debug!("ChannelPointsRedemptionHandler: Processing redemption: {:?}", evt);

        let platform = Platform::Twitch;
        let redeem_id = &evt.reward.id;
        let user_id = &evt.user_id;
        let user_display = &evt.user_name;
        let user_input = &evt.user_input;

        info!(
            "ChannelPointsRedemptionHandler: Channel points redemption - reward: '{}' ({}), user: {} ({})",
            evt.reward.title, redeem_id, user_display, user_id
        );

        // Look up user
        let user = ctx.user_service
            .get_or_create_user(&platform.to_string(), user_id, Some(user_display))
            .await?;

        // Check if this redeem is in our database
        match ctx.redeem_service.redeem_repo.get_redeem_by_reward_id(&platform.to_string(), redeem_id).await {
            Ok(Some(db_redeem)) => {
                info!(
                    "ChannelPointsRedemptionHandler: Found redeem in database: {} (active: {})",
                    db_redeem.reward_name, db_redeem.is_active
                );

                if db_redeem.is_active {
                    // Create a Redemption struct for the redeem handler
                    let redemption = Redemption {
                        broadcaster_id: evt.broadcaster_user_id.clone(),
                        broadcaster_login: Some(evt.broadcaster_user_login.clone()),
                        broadcaster_name: Some(evt.broadcaster_user_name.clone()),
                        id: evt.id.clone(),
                        user_id: evt.user_id.clone(),
                        user_name: Some(evt.user_name.clone()),
                        user_login: Some(evt.user_login.clone()),
                        user_input: evt.user_input.clone(),
                        status: evt.status.clone(),
                        redeemed_at: evt.redeemed_at.to_rfc3339(),
                        reward: RedemptionReward {
                            id: evt.reward.id.clone(),
                            title: evt.reward.title.clone(),
                            prompt: evt.reward.prompt.clone(),
                            cost: evt.reward.cost as u64,
                        },
                    };
                    
                    // Process the redemption using the handle_incoming_redeem method
                    if let Err(e) = ctx.redeem_service
                        .handle_incoming_redeem(
                            &platform.to_string(),
                            redeem_id,
                            user.user_id,
                            "twitch", // channel name
                            &redemption
                        )
                        .await
                    {
                        error!("ChannelPointsRedemptionHandler: Failed to process redeem '{}': {:?}", 
                               db_redeem.reward_name, e);
                    } else {
                        info!("ChannelPointsRedemptionHandler: Successfully processed redeem '{}'", 
                              db_redeem.reward_name);
                    }
                } else {
                    debug!("ChannelPointsRedemptionHandler: Redeem '{}' is disabled, skipping", 
                           db_redeem.reward_name);
                }
            }
            Ok(None) => {
                debug!("ChannelPointsRedemptionHandler: Redeem '{}' not found in database", redeem_id);
            }
            Err(e) => {
                error!("ChannelPointsRedemptionHandler: Error looking up redeem '{}': {:?}", redeem_id, e);
            }
        }

        Ok(())
    }
}