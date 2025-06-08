use async_trait::async_trait;
use tracing::{debug, info};
use maowbot_common::models::platform::Platform;
use maowbot_common::models::discord::{DiscordEmbed, DiscordColor};

use crate::Error;
use crate::eventbus::{BotEvent, TwitchEventSubData};
use crate::platforms::twitch_eventsub::events::StreamOffline;
use crate::services::event_context::EventContext;
use crate::services::event_handler::{EventHandler, TypedEventHandler};

/// Handler for Twitch stream.offline events
pub struct StreamOfflineHandler;

impl StreamOfflineHandler {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl EventHandler for StreamOfflineHandler {
    fn id(&self) -> &str {
        "twitch.stream.offline"
    }

    fn event_types(&self) -> Vec<String> {
        vec!["stream.offline".to_string()]
    }

    fn platforms(&self) -> Vec<Platform> {
        vec![Platform::Twitch, Platform::TwitchEventSub]
    }

    async fn handle(&self, event: &BotEvent, ctx: &EventContext) -> Result<bool, Error> {
        match event {
            BotEvent::TwitchEventSub(TwitchEventSubData::StreamOffline(evt)) => {
                self.handle_typed(evt, ctx).await?;
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    fn priority(&self) -> i32 {
        50 // Higher priority for stream notifications
    }
}

#[async_trait]
impl TypedEventHandler<StreamOffline> for StreamOfflineHandler {
    async fn handle_typed(&self, evt: &StreamOffline, ctx: &EventContext) -> Result<(), Error> {
        debug!("StreamOfflineHandler: Processing stream.offline event: {:?}", evt);

        // 1) Retrieve the broadcaster credential for Twitch
        let broadcaster_cred_opt = ctx.credentials_repo
            .get_broadcaster_credential(&Platform::Twitch)
            .await?;

        if let Some(broadcaster_cred) = broadcaster_cred_opt {
            let broadcaster_name = broadcaster_cred.user_name.clone();

            // 2) Look up the Discord event config for "stream.offline"
            if let Some(config) = ctx.discord_repo.get_event_config_by_name("stream.offline").await? {
                // Determine which account to send from
                let account_name = if let Some(cred_id) = config.respond_with_credential {
                    if let Some(dc_cred) = ctx.credentials_repo
                        .get_credential_by_id(cred_id)
                        .await?
                    {
                        dc_cred.user_name
                    } else {
                        "unknown_Us3r".to_string()
                    }
                } else {
                    "unknown_Us3r".to_string()
                };

                // Create a simple embed for stream offline notification
                let mut embed = DiscordEmbed::new();
                embed.title = Some(format!("{} has ended their stream", broadcaster_name));
                embed.description = Some("Thanks for watching!".to_string());
                embed.color = Some(DiscordColor::DARK_GREY);
                embed.timestamp = Some(chrono::Utc::now());

                info!("StreamOfflineHandler: Sending Discord notification for stream offline from account '{}'", account_name);

                // 3) Send the Discord embed
                ctx.platform_manager
                    .send_discord_embed(
                        &account_name,
                        &config.guild_id,
                        &config.channel_id,
                        &embed,
                        None // No ping for offline notifications
                    )
                    .await?;
            }
        }

        Ok(())
    }
}