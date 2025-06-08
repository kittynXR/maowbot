pub mod message;
pub mod presence_update;
pub mod interaction;
pub mod member_update;
pub mod ready;
pub mod configured_action;

use std::sync::Arc;
use crate::services::event_registry::EventHandlerRegistry;
use crate::Error;

/// Register all Discord event handlers
pub async fn register_handlers(registry: &EventHandlerRegistry) -> Result<(), Error> {
    // Message events
    registry.register(Arc::new(message::DiscordMessageHandler::new())).await?;
    
    // Presence update events (for live roles)
    registry.register(Arc::new(presence_update::PresenceUpdateHandler::new())).await?;
    registry.register(Arc::new(presence_update::TwitchLiveRoleHandler::new())).await?;
    
    // Interaction events (slash commands)
    registry.register(Arc::new(interaction::InteractionHandler::new())).await?;
    
    // Member update events
    registry.register(Arc::new(member_update::MemberUpdateHandler::new())).await?;
    registry.register(Arc::new(member_update::WelcomeMessageHandler::new("welcome"))).await?;
    
    // Ready event
    registry.register(Arc::new(ready::ReadyHandler::new())).await?;
    registry.register(Arc::new(ready::BotStatusHandler::new("Watching chat", "watching"))).await?;
    
    // Configured action handlers
    registry.register(Arc::new(configured_action::DiscordConfiguredActionHandler::new())).await?;
    registry.register(Arc::new(configured_action::MemberEventNotificationHandler::new())).await?;
    
    Ok(())
}