// File: src/platforms/manager.rs

use std::sync::Arc;
use tracing::{info, error};
// ADD:
use crate::platforms::PlatformIntegration; // so .connect() etc. are recognized

use crate::eventbus::EventBus;
use crate::services::message_service::MessageService;
use crate::services::user_service::UserService;
use crate::Error;

use crate::platforms::discord::runtime::{DiscordPlatform};
use crate::platforms::twitch_helix::runtime::{TwitchPlatform};
use crate::platforms::vrchat::runtime::{VRChatPlatform};

/// PlatformManager is responsible for starting/stopping each platform runtime,
/// then funneling all inbound messages into our `MessageService`.
pub struct PlatformManager {
    /// Each runtime might have its own handle so we can stop them if needed
    // e.g. discord: DiscordPlatform, twitch_helix: TwitchPlatform, etc.

    /// Shared message service
    message_svc: Arc<MessageService>,

    /// Shared user service
    user_svc: Arc<UserService>,

    /// Event bus
    event_bus: Arc<EventBus>,
}

impl PlatformManager {
    pub fn new(
        message_svc: Arc<MessageService>,
        user_svc: Arc<UserService>,
        event_bus: Arc<EventBus>,
    ) -> Self {
        Self {
            message_svc,
            user_svc,
            event_bus,
        }
    }

    /// Start each platform’s runtime tasks in the background.
    /// In a real application, you might pass credentials or config to each platform type.
    pub async fn start_all_platforms(&self) -> Result<(), Error> {
        // Example: spawn Discord
        let discord = DiscordPlatform::new(/* pass credentials, config, etc. */);
        self.start_discord_runtime(discord);

        // Example: spawn Twitch
        let twitch = TwitchPlatform::new(/* pass credentials, config, etc. */);
        self.start_twitch_runtime(twitch);

        // Example: spawn VRChat
        let vrchat = VRChatPlatform::new(/* pass credentials, config, etc. */);
        self.start_vrchat_runtime(vrchat);

        Ok(())
    }

    fn start_discord_runtime(&self, mut platform: DiscordPlatform) {
        let message_svc = Arc::clone(&self.message_svc);
        let user_svc = Arc::clone(&self.user_svc);
        let event_bus = Arc::clone(&self.event_bus);

        // Spawn a background task
        tokio::spawn(async move {
            if let Err(err) = platform.connect().await {
                error!("[Discord] connect error: {:?}", err);
                return;
            }
            info!("[Discord] Connected.");

            // Now run a loop reading from Discord events.
            // Pseudocode: we might do:
            while let Some(msg_event) = platform.next_message_event().await {
                let platform_name = "discord";
                let channel = msg_event.channel.clone();
                let user_platform_id = msg_event.user_id.clone();
                let text = msg_event.text.clone();

                // 1) Get or create user from DB
                let user = match user_svc
                    .get_or_create_user(platform_name, &user_platform_id, Some(&msg_event.username))
                    .await
                {
                    Ok(u) => u,
                    Err(e) => {
                        error!("[Discord] failed to get_or_create_user: {:?}", e);
                        continue;
                    }
                };

                // 2) Handle message caching + event bus logging
                if let Err(e) = message_svc.process_incoming_message(
                    platform_name,
                    &channel,
                    &user.user_id,
                    &text,
                ).await {
                    error!("[Discord] process_incoming_message failed: {:?}", e);
                }
            }

            info!("[Discord] Runtime ended.");
        });
    }

    fn start_twitch_runtime(&self, mut platform: TwitchPlatform) {
        let message_svc = Arc::clone(&self.message_svc);
        let user_svc = Arc::clone(&self.user_svc);
        let event_bus = Arc::clone(&self.event_bus);

        tokio::spawn(async move {
            if let Err(err) = platform.connect().await {
                error!("[Twitch] connect error: {:?}", err);
                return;
            }
            info!("[Twitch] Connected.");

            // In a real app, we might have an event loop reading chat messages
            while let Some(twitch_msg) = platform.next_message_event().await {
                let platform_name = "twitch_helix";
                let channel = twitch_msg.channel.clone();
                let user_platform_id = twitch_msg.user_id.clone();
                let text = twitch_msg.text.clone();

                let user = match user_svc
                    .get_or_create_user(platform_name, &user_platform_id, Some(&twitch_msg.display_name))
                    .await
                {
                    Ok(u) => u,
                    Err(e) => {
                        error!("[Twitch] failed to get_or_create_user: {:?}", e);
                        continue;
                    }
                };

                // Then store in cache / publish via event bus
                if let Err(e) = message_svc.process_incoming_message(
                    platform_name,
                    &channel,
                    &user.user_id,
                    &text,
                ).await {
                    error!("[Twitch] process_incoming_message failed: {:?}", e);
                }
            }

            info!("[Twitch] Runtime ended.");
        });
    }

    fn start_vrchat_runtime(&self, mut platform: VRChatPlatform) {
        let message_svc = Arc::clone(&self.message_svc);
        let user_svc = Arc::clone(&self.user_svc);
        let event_bus = Arc::clone(&self.event_bus);

        tokio::spawn(async move {
            if let Err(err) = platform.connect().await {
                error!("[VRChat] connect error: {:?}", err);
                return;
            }
            info!("[VRChat] Connected.");

            while let Some(vrc_evt) = platform.next_message_event().await {
                let platform_name = "vrchat";
                let channel = "instanceOrRoomId"; // VRChat might not have channels the same way
                let user_platform_id = vrc_evt.user_id.clone();
                let text = vrc_evt.text.clone();

                let user = match user_svc
                    .get_or_create_user(platform_name, &user_platform_id, Some(&vrc_evt.vrchat_display_name))
                    .await
                {
                    Ok(u) => u,
                    Err(e) => {
                        error!("[VRChat] failed to get_or_create_user: {:?}", e);
                        continue;
                    }
                };

                if let Err(e) = message_svc.process_incoming_message(
                    platform_name,
                    channel,
                    &user.user_id,
                    &text,
                ).await {
                    error!("[VRChat] process_incoming_message failed: {:?}", e);
                }
            }

            info!("[VRChat] Runtime ended.");
        });
    }
}
