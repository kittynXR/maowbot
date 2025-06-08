use async_trait::async_trait;
use tracing::{debug, info};
use maowbot_common::models::platform::Platform;
use maowbot_common::models::discord::{DiscordEmbed, DiscordEmbedAuthor, DiscordEmbedThumbnail, DiscordColor, DiscordEmbedField};

use crate::Error;
use crate::eventbus::{BotEvent, TwitchEventSubData};
use crate::platforms::twitch_eventsub::events::StreamOnline;
use crate::platforms::twitch::requests::stream::fetch_stream_details;
use crate::services::event_context::EventContext;
use crate::services::event_handler::{EventHandler, TypedEventHandler};
use crate::tasks::redeem_sync;

/// Handler for Twitch stream.online events
pub struct StreamOnlineHandler;

impl StreamOnlineHandler {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl EventHandler for StreamOnlineHandler {
    fn id(&self) -> &str {
        "twitch.stream.online"
    }

    fn event_types(&self) -> Vec<String> {
        vec!["stream.online".to_string()]
    }

    fn platforms(&self) -> Vec<Platform> {
        vec![Platform::Twitch, Platform::TwitchEventSub]
    }

    async fn handle(&self, event: &BotEvent, ctx: &EventContext) -> Result<bool, Error> {
        match event {
            BotEvent::TwitchEventSub(TwitchEventSubData::StreamOnline(evt)) => {
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
impl TypedEventHandler<StreamOnline> for StreamOnlineHandler {
    async fn handle_typed(&self, evt: &StreamOnline, ctx: &EventContext) -> Result<(), Error> {
        debug!("StreamOnlineHandler: Processing stream.online event: {:?}", evt);

        // 1) Retrieve the broadcaster credential for Twitch
        let broadcaster_cred_opt = ctx.credentials_repo
            .get_broadcaster_credential(&Platform::Twitch)
            .await?;

        if let Some(broadcaster_cred) = broadcaster_cred_opt {
            let broadcaster_name = broadcaster_cred.user_name.clone();
            let link = format!("https://twitch.tv/{}", broadcaster_name);

            // 2) Fetch additional stream details from Twitch using real-time API data
            let twitch_client = ctx.platform_manager
                .get_twitch_client()
                .await
                .ok_or_else(|| Error::Platform("Twitch client not available".into()))?;

            let details = fetch_stream_details(&twitch_client, &broadcaster_name).await?;

            // 3) Look up the Discord event config for "stream.online"
            if let Some(config) = ctx.discord_repo.get_event_config_by_name("stream.online").await? {
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

                // If any ping roles are set, format them as Discord role mentions
                let ping_str = if let Some(roles) = &config.ping_roles {
                    if !roles.is_empty() {
                        roles.iter()
                            .map(|r| format!("<@&{}>", r))
                            .collect::<Vec<_>>()
                            .join(" ")
                    } else {
                        "".to_string()
                    }
                } else {
                    "".to_string()
                };

                // Create the embed for the stream announcement
                let mut embed = DiscordEmbed::new();
                embed.title = Some(format!("{} is live on Twitch!", details.broadcaster_name));
                embed.description = Some(details.stream_title);
                embed.url = Some(link.clone());
                embed.color = Some(DiscordColor::TWITCH_PURPLE);

                // Set thumbnail to game image
                embed.thumbnail = Some(DiscordEmbedThumbnail {
                    url: details.game_thumbnail
                });

                // Set author with streamer info and profile picture
                embed.author = Some(DiscordEmbedAuthor {
                    name: details.broadcaster_name.clone(),
                    url: Some(link.clone()),
                    icon_url: Some(details.pfp)
                });

                // Add game as a field
                embed.fields.push(DiscordEmbedField {
                    name: "Playing".to_string(),
                    value: details.game,
                    inline: true
                });

                // Current time as a timestamp
                embed.timestamp = Some(chrono::Utc::now());

                info!("StreamOnlineHandler: Sending Discord embed for stream announcement from account '{}'", account_name);

                // 4) Send the Discord embed with optional ping content
                ctx.platform_manager
                    .send_discord_embed(
                        &account_name,
                        &config.guild_id,
                        &config.channel_id,
                        &embed,
                        if ping_str.is_empty() { None } else { Some(&ping_str) }
                    )
                    .await?;
            }
        }

        // 5) Optionally, perform redeem sync
        redeem_sync::sync_channel_redeems(
            &ctx.redeem_service,
            &ctx.platform_manager,
            &ctx.user_service,
            ctx.bot_config_repo.as_ref(),
            false,
        )
        .await?;

        Ok(())
    }
}