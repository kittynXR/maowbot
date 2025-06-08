use async_trait::async_trait;
use tracing::{debug, info, warn};
use maowbot_common::models::platform::Platform;

use crate::Error;
use crate::eventbus::BotEvent;
use crate::services::event_context::EventContext;
use crate::services::event_handler::EventHandler;

/// Handler for Discord presence update events (manages live roles)
pub struct PresenceUpdateHandler;

impl PresenceUpdateHandler {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl EventHandler for PresenceUpdateHandler {
    fn id(&self) -> &str {
        "discord.presence_update"
    }

    fn event_types(&self) -> Vec<String> {
        vec!["presence.update".to_string()]
    }

    fn platforms(&self) -> Vec<Platform> {
        vec![Platform::Discord]
    }

    async fn handle(&self, event: &BotEvent, ctx: &EventContext) -> Result<bool, Error> {
        // Note: In the current architecture, presence updates are handled directly
        // in the Discord runtime. This handler is a placeholder for when we
        // create a proper DiscordEvent enum in the BotEvent system.
        
        // For now, this would require adding a new BotEvent variant like:
        // BotEvent::DiscordPresenceUpdate { guild_id, user_id, activities, ... }
        
        debug!("PresenceUpdateHandler: Would handle presence update event");
        Ok(false)
    }

    fn priority(&self) -> i32 {
        80 // Higher priority for role management
    }
}

/// Specialized handler for Twitch streaming presence updates
pub struct TwitchLiveRoleHandler;

impl TwitchLiveRoleHandler {
    pub fn new() -> Self {
        Self
    }

    async fn handle_streaming_presence(
        &self,
        ctx: &EventContext,
        guild_id: &str,
        user_id: &str,
        is_streaming: bool,
    ) -> Result<(), Error> {
        // Check if this guild has a live role configured
        match ctx.discord_repo.get_live_role(guild_id).await? {
            Some(live_role) => {
                let role_id = &live_role.role_id;
                
                if is_streaming {
                    info!(
                        "TwitchLiveRoleHandler: User {} started streaming in guild {}, adding role {}",
                        user_id, guild_id, role_id
                    );
                    
                    // Add the live role
                    // TODO: Implement add_discord_role in PlatformManager
                    // if let Err(e) = ctx.platform_manager
                    //     .add_discord_role("bot", guild_id, user_id, role_id)
                    //     .await
                    // {
                    //     warn!("Failed to add live role: {:?}", e);
                    // }
                    info!("Would add role {} to user {} in guild {}", role_id, user_id, guild_id);
                } else {
                    info!(
                        "TwitchLiveRoleHandler: User {} stopped streaming in guild {}, removing role {}",
                        user_id, guild_id, role_id
                    );
                    
                    // Remove the live role
                    // TODO: Implement remove_discord_role in PlatformManager
                    // if let Err(e) = ctx.platform_manager
                    //     .remove_discord_role("bot", guild_id, user_id, role_id)
                    //     .await
                    // {
                    //     warn!("Failed to remove live role: {:?}", e);
                    // }
                    info!("Would remove role {} from user {} in guild {}", role_id, user_id, guild_id);
                }
            }
            None => {
                debug!("No live role configured for guild {}", guild_id);
            }
        }
        
        Ok(())
    }
}

#[async_trait]
impl EventHandler for TwitchLiveRoleHandler {
    fn id(&self) -> &str {
        "discord.twitch_live_role"
    }

    fn event_types(&self) -> Vec<String> {
        vec!["presence.update".to_string()]
    }

    fn platforms(&self) -> Vec<Platform> {
        vec![Platform::Discord]
    }

    async fn handle(&self, event: &BotEvent, ctx: &EventContext) -> Result<bool, Error> {
        // This would handle the presence update event when we add it to BotEvent
        // For now, the logic is in the Discord runtime
        debug!("TwitchLiveRoleHandler: Would handle Twitch streaming presence");
        Ok(false)
    }

    fn priority(&self) -> i32 {
        70 // Higher priority than general presence handler
    }
}