pub mod stream_online;
pub mod stream_offline;
pub mod channel_points;

use std::sync::Arc;
use crate::services::event_registry::EventHandlerRegistry;
use crate::Error;

/// Register all Twitch event handlers
pub async fn register_handlers(registry: &EventHandlerRegistry) -> Result<(), Error> {
    // Stream events
    registry.register(Arc::new(stream_online::StreamOnlineHandler::new())).await?;
    registry.register(Arc::new(stream_offline::StreamOfflineHandler::new())).await?;
    
    // Channel points
    registry.register(Arc::new(channel_points::ChannelPointsRedemptionHandler::new())).await?;
    
    // Future handlers can be added here as they're implemented
    
    Ok(())
}