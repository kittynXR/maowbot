use std::sync::Arc;
use tracing::{debug, error, info};
use crate::eventbus::{EventBus, BotEvent};
use crate::services::event_context::EventContext;
use crate::services::event_registry::EventHandlerRegistry;

/// Service that listens for Discord events on the EventBus and dispatches
/// them to registered handlers using the EventHandlerRegistry.
pub struct DiscordEventService {
    event_bus: Arc<EventBus>,
    registry: Arc<EventHandlerRegistry>,
    context: Arc<EventContext>,
}

impl DiscordEventService {
    pub fn new(
        event_bus: Arc<EventBus>,
        registry: Arc<EventHandlerRegistry>,
        context: Arc<EventContext>,
    ) -> Self {
        Self {
            event_bus,
            registry,
            context,
        }
    }

    /// Spawn a task to listen to the event bus and dispatch Discord events
    pub async fn start(&self) {
        let mut rx = self.event_bus.subscribe(None).await;

        info!("DiscordEventService: Started, listening on EventBus");

        while let Some(event) = rx.recv().await {
            // Process Discord-specific events
            match &event {
                BotEvent::ChatMessage { platform, .. } if platform == "discord" => {
                    debug!("DiscordEventService: Received Discord chat message");
                    self.dispatch_event(event).await;
                }
                // When we add more Discord-specific BotEvent variants, handle them here
                // BotEvent::DiscordPresenceUpdate { .. } => { ... }
                // BotEvent::DiscordInteraction { .. } => { ... }
                // BotEvent::DiscordMemberUpdate { .. } => { ... }
                // BotEvent::DiscordReady { .. } => { ... }
                _ => {
                    // Ignore non-Discord events
                }
            }
        }
        
        info!("DiscordEventService: Shutting down listener loop");
    }

    /// Dispatch an event to all registered handlers
    async fn dispatch_event(&self, event: BotEvent) {
        // Get all handlers that can process this event
        let handlers = self.registry.get_handlers_for_event(&event).await;
        
        if handlers.is_empty() {
            debug!("DiscordEventService: No handlers registered for event type");
        } else {
            debug!("DiscordEventService: Found {} handlers for event", handlers.len());
            
            // Execute handlers in priority order
            for handler in handlers {
                debug!("DiscordEventService: Executing handler '{}'", handler.id());
                
                match handler.handle(&event, &self.context).await {
                    Ok(true) => {
                        debug!("DiscordEventService: Handler '{}' processed event successfully", handler.id());
                    }
                    Ok(false) => {
                        debug!("DiscordEventService: Handler '{}' skipped event", handler.id());
                    }
                    Err(e) => {
                        error!("DiscordEventService: Handler '{}' failed: {:?}", handler.id(), e);
                    }
                }
            }
        }
    }
}