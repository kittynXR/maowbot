use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{debug, error, info};
use crate::eventbus::{EventBus, BotEvent, TwitchEventSubData};

use super::event_actions::{
    channel::update as channel_update_actions,
    stream::online as stream_online_actions,
    stream::offline as stream_offline_actions,
};

/// The EventSubService will subscribe to the EventBus, look for `BotEvent::TwitchEventSub`,
/// and dispatch to the appropriate event_actions submodule.
pub struct EventSubService {
    event_bus: Arc<EventBus>,
}

impl EventSubService {
    pub fn new(event_bus: Arc<EventBus>) -> Self {
        Self { event_bus }
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
                        // For each event, call a submodule function
                        TwitchEventSubData::ChannelUpdate(ev) => {
                            if let Err(e) = channel_update_actions::handle_channel_update(ev).await {
                                error!("Error handling channel.update: {:?}", e);
                            }
                        },
                        TwitchEventSubData::StreamOnline(ev) => {
                            if let Err(e) = stream_online_actions::handle_stream_online(ev).await {
                                error!("Error handling stream.online: {:?}", e);
                            }
                        },
                        TwitchEventSubData::StreamOffline(ev) => {
                            if let Err(e) = stream_offline_actions::handle_stream_offline(ev).await {
                                error!("Error handling stream.offline: {:?}", e);
                            }
                        },

                        // If you implement more events, add them here:
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
