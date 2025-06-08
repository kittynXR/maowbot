use async_trait::async_trait;
use crate::eventbus::BotEvent;
use crate::services::event_context::EventContext;
use crate::Error;
use maowbot_common::models::platform::Platform;

/// Base trait for all event handlers in the system.
/// Handlers process BotEvents and can access services through EventContext.
#[async_trait]
pub trait EventHandler: Send + Sync {
    /// Returns a unique identifier for this handler
    fn id(&self) -> &str;
    
    /// Returns the event type(s) this handler can process
    fn event_types(&self) -> Vec<String>;
    
    /// Returns the platform(s) this handler is for (twitch, discord, etc)
    fn platforms(&self) -> Vec<Platform>;
    
    /// Process the event. Return Ok(true) if handled, Ok(false) if skipped.
    async fn handle(&self, event: &BotEvent, ctx: &EventContext) -> Result<bool, Error>;
    
    /// Priority for this handler (lower numbers run first)
    fn priority(&self) -> i32 {
        100 // default priority
    }
    
    /// Whether this handler is enabled
    fn is_enabled(&self) -> bool {
        true
    }
}

/// Typed event handler for specific event types.
/// This provides type safety when implementing handlers for known event types.
#[async_trait]
pub trait TypedEventHandler<T>: Send + Sync {
    /// Handle a specific typed event
    async fn handle_typed(&self, event: &T, ctx: &EventContext) -> Result<(), Error>;
}

/// Metadata about an event handler for registration and management
#[derive(Debug, Clone)]
pub struct EventHandlerInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    pub platforms: Vec<Platform>,
    pub event_types: Vec<String>,
    pub priority: i32,
    pub enabled: bool,
}

/// Result of handling an event
#[derive(Debug)]
pub enum EventHandlerResult {
    /// Event was handled successfully
    Handled,
    /// Event was not applicable to this handler
    Skipped,
    /// Event handling failed with error
    Failed(Error),
}

/// Extension trait for EventHandler to provide convenience methods
#[async_trait]
pub trait EventHandlerExt: EventHandler {
    /// Check if this handler can process the given event
    fn can_handle(&self, event: &BotEvent) -> bool {
        // Default implementation - handlers can override for custom logic
        match event {
            BotEvent::ChatMessage { platform, .. } => {
                let platform_enum = Platform::from_string(platform);
                self.platforms().contains(&platform_enum)
            }
            BotEvent::TwitchEventSub(_) => {
                self.platforms().contains(&Platform::Twitch) || 
                self.platforms().contains(&Platform::TwitchEventSub)
            }
            _ => false,
        }
    }
}

impl<T: EventHandler + ?Sized> EventHandlerExt for T {}