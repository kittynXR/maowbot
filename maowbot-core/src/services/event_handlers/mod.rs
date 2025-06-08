pub mod twitch;
pub mod discord;

use std::sync::Arc;
use crate::services::event_registry::EventHandlerRegistry;
use crate::Error;

/// Register all built-in event handlers with the registry
pub async fn register_builtin_handlers(registry: &EventHandlerRegistry) -> Result<(), Error> {
    // Register Twitch handlers
    twitch::register_handlers(registry).await?;
    
    // Register Discord handlers
    discord::register_handlers(registry).await?;
    
    Ok(())
}