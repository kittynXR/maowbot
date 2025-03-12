use std::sync::Arc;
use tracing::{debug, error, info};
use maowbot_common::traits::repository_traits::BotConfigRepository;
use crate::eventbus::{EventBus, BotEvent, TwitchEventSubData};
use crate::platforms::manager::PlatformManager;
use crate::services::RedeemService;
use crate::services::user_service::UserService;
use super::event_actions::{
    channel::update as channel_update_actions,
    stream::online as stream_online_actions,
    stream::offline as stream_offline_actions,
    channel::points as channel_points_actions,
};

/// The EventSubService will subscribe to the EventBus, look for `BotEvent::TwitchEventSub`,
/// and dispatch to the appropriate event_actions submodule.
pub struct EventSubService {
    event_bus: Arc<EventBus>,
    pub redeem_service: Arc<RedeemService>,
    pub user_service: Arc<UserService>,
    pub platform_manager: Arc<PlatformManager>,
    pub bot_config_repo: Arc<dyn BotConfigRepository + Send + Sync>,
}

impl EventSubService {
    pub fn new(
        event_bus: Arc<EventBus>,
        redeem_service: Arc<RedeemService>,
        user_service: Arc<UserService>,
        platform_manager: Arc<PlatformManager>,
        bot_config_repo: Arc<dyn BotConfigRepository + Send + Sync>,
    ) -> Self {
        Self {
            event_bus,
            redeem_service,
            user_service,
            platform_manager,
            bot_config_repo,
        }
    }

    /// Spawn a task to listen to the event bus and handle EventSub-related events.
    /// Or, you can just run this inline in your runtime.
    pub async fn start(&self) {
        let mut rx = self.event_bus.subscribe(None).await;

        info!("EventSubService started, listening on EventBus.");

        while let Some(event) = rx.recv().await {
            match event {
                BotEvent::TwitchEventSub(twitch_evt) => {
                    // Dispatch by subscription type
                    match twitch_evt {
                        TwitchEventSubData::ChannelUpdate(ev) => {
                            if let Err(e) = channel_update_actions::handle_channel_update(ev).await {
                                error!("Error handling channel.update: {:?}", e);
                            }
                        },
                        TwitchEventSubData::StreamOnline(ev) => {
                            if let Err(e) = stream_online_actions::handle_stream_online(
                                ev,
                                &*self.redeem_service,
                                &*self.platform_manager,
                                &*self.user_service,
                                &*self.bot_config_repo
                            ).await {
                                error!("Error handling stream.online: {:?}", e);
                            }
                        },
                        TwitchEventSubData::StreamOffline(ev) => {
                            if let Err(e) = stream_offline_actions::handle_stream_offline(
                                ev,
                                &*self.redeem_service,
                                &*self.platform_manager,
                                &*self.user_service,
                                &*self.bot_config_repo
                            ).await {
                                error!("Error handling stream.offline: {:?}", e);
                            }
                        },

                        // ----------------- NEW MATCH ARM ---------------------
                        TwitchEventSubData::ChannelPointsCustomRewardRedemptionAdd(ev) => {
                            if let Err(e) = channel_points_actions::handle_custom_reward_redemption_add(
                                ev,
                                &*self.redeem_service,
                                &*self.user_service
                            ).await
                            {
                                error!("Error handling custom_reward_redemption.add: {:?}", e);
                            }
                        }
                        // -----------------------------------------------------

                        // If you add more channel points events, do similar arms here:
                        //    ChannelPointsCustomRewardRedemptionUpdate(ev) => { ... }
                        //    ChannelPointsCustomRewardAdd(ev) => { ... }
                        // etc.

                        // If not matched, log "ignoring unhandled variant"
                        _ => {
                            debug!("(EventSubService) Ignoring unhandled TwitchEventSubData variant: {:?}", twitch_evt);
                        }
                    }
                },
                _ => {
                    // We ignore all other BotEvents
                }
            }
        }
        info!("EventSubService: shutting down listener loop.");
    }
}