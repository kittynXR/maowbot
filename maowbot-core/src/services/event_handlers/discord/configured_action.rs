use async_trait::async_trait;
use tracing::{debug, info, error};
use maowbot_common::models::platform::Platform;
use maowbot_common::models::discord::{DiscordEmbed, DiscordEmbedField, DiscordColor};

use crate::Error;
use crate::eventbus::BotEvent;
use crate::services::event_context::EventContext;
use crate::services::event_handler::EventHandler;

/// Handler that processes Discord events based on configured actions
/// Similar to how Twitch events can trigger Discord notifications
pub struct DiscordConfiguredActionHandler;

impl DiscordConfiguredActionHandler {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl EventHandler for DiscordConfiguredActionHandler {
    fn id(&self) -> &str {
        "discord.configured_action"
    }

    fn event_types(&self) -> Vec<String> {
        vec![
            "message.create".to_string(),
            "member.add".to_string(),
            "member.remove".to_string(),
            "voice.state_update".to_string(),
        ]
    }

    fn platforms(&self) -> Vec<Platform> {
        vec![Platform::Discord]
    }

    async fn handle(&self, event: &BotEvent, ctx: &EventContext) -> Result<bool, Error> {
        // This demonstrates how Discord events could trigger configured actions
        // based on event configurations stored in the database
        
        match event {
            BotEvent::ChatMessage { platform, channel, user, text, .. } if platform == "discord" => {
                // Check if there's a configured action for Discord messages
                if let Some(config) = ctx.discord_repo.get_event_config_by_name("discord.message").await? {
                    self.handle_message_action(ctx, &config, channel, user, text).await?;
                    Ok(true)
                } else {
                    Ok(false)
                }
            }
            // Future: Handle other Discord event types when added to BotEvent
            _ => Ok(false),
        }
    }

    fn priority(&self) -> i32 {
        90 // Normal priority
    }
}

impl DiscordConfiguredActionHandler {
    /// Handle configured actions for Discord messages
    async fn handle_message_action(
        &self,
        ctx: &EventContext,
        config: &maowbot_common::models::discord::DiscordEventConfigRecord,
        source_channel: &str,
        user: &str,
        text: &str,
    ) -> Result<(), Error> {
        debug!("DiscordConfiguredActionHandler: Processing message action");
        
        // Example: Mirror messages to another channel
        if config.event_name == "discord.message.mirror" {
            // Send to configured channel
            let account_name = if let Some(cred_id) = config.respond_with_credential {
                if let Some(cred) = ctx.credentials_repo.get_credential_by_id(cred_id).await? {
                    cred.user_name
                } else {
                    "bot".to_string()
                }
            } else {
                "bot".to_string()
            };
            
            let mirror_message = format!("Mirror from <#{}> by <@{}>: {}", source_channel, user, text);
            
            ctx.platform_manager
                .send_discord_message(&account_name, &config.guild_id, &config.channel_id, &mirror_message)
                .await?;
        }
        
        Ok(())
    }
}

/// Handler for Discord member join/leave events with embed notifications
pub struct MemberEventNotificationHandler;

impl MemberEventNotificationHandler {
    pub fn new() -> Self {
        Self
    }
    
    async fn send_member_notification(
        &self,
        ctx: &EventContext,
        event_type: &str,
        guild_id: &str,
        user_id: &str,
        username: &str,
    ) -> Result<(), Error> {
        // Check for configured notification channel
        if let Some(config) = ctx.discord_repo.get_event_config_by_name(event_type).await? {
            let mut embed = DiscordEmbed::new();
            
            match event_type {
                "member.join" => {
                    embed.title = Some(format!("Welcome {}!", username));
                    embed.description = Some(format!("<@{}> has joined the server!", user_id));
                    embed.color = Some(DiscordColor::GREEN);
                    embed.timestamp = Some(chrono::Utc::now());
                    
                    embed.fields.push(DiscordEmbedField {
                        name: "User ID".to_string(),
                        value: user_id.to_string(),
                        inline: true,
                    });
                }
                "member.leave" => {
                    embed.title = Some(format!("Goodbye {}", username));
                    embed.description = Some(format!("<@{}> has left the server", user_id));
                    embed.color = Some(DiscordColor::RED);
                    embed.timestamp = Some(chrono::Utc::now());
                }
                _ => return Ok(()),
            }
            
            // Send embed to configured channel
            let account_name = if let Some(cred_id) = config.respond_with_credential {
                if let Some(cred) = ctx.credentials_repo.get_credential_by_id(cred_id).await? {
                    cred.user_name
                } else {
                    "bot".to_string()
                }
            } else {
                "bot".to_string()
            };
            
            info!(
                "MemberEventNotificationHandler: Sending {} notification for {} to channel {}",
                event_type, username, config.channel_id
            );
            
            ctx.platform_manager
                .send_discord_embed(
                    &account_name,
                    &config.guild_id,
                    &config.channel_id,
                    &embed,
                    None,
                )
                .await?;
        }
        
        Ok(())
    }
}

#[async_trait]
impl EventHandler for MemberEventNotificationHandler {
    fn id(&self) -> &str {
        "discord.member_notification"
    }

    fn event_types(&self) -> Vec<String> {
        vec!["member.join".to_string(), "member.leave".to_string()]
    }

    fn platforms(&self) -> Vec<Platform> {
        vec![Platform::Discord]
    }

    async fn handle(&self, event: &BotEvent, ctx: &EventContext) -> Result<bool, Error> {
        // This would handle member events when added to BotEvent
        debug!("MemberEventNotificationHandler: Would send member event notification");
        Ok(false)
    }

    fn priority(&self) -> i32 {
        85
    }
}