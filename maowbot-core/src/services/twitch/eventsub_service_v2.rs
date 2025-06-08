// File: maowbot-core/src/services/twitch/eventsub_service_v2.rs
// This is the refactored EventSubService using the new event handler system

use std::sync::Arc;
use tracing::{debug, error, info, warn};
use crate::eventbus::{EventBus, BotEvent};
use crate::services::event_context::EventContext;
use crate::services::event_registry::EventHandlerRegistry;
use crate::services::event_handlers;

/// The refactored EventSubService that uses the EventHandlerRegistry
/// for dynamic event handling instead of hardcoded match statements.
pub struct EventSubServiceV2 {
    event_bus: Arc<EventBus>,
    registry: Arc<EventHandlerRegistry>,
    context: Arc<EventContext>,
}

impl EventSubServiceV2 {
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

    /// Initialize the service by registering built-in handlers
    pub async fn initialize(&self) -> Result<(), crate::Error> {
        info!("EventSubServiceV2: Registering built-in event handlers");
        event_handlers::register_builtin_handlers(&self.registry).await?;
        
        let handlers = self.registry.list_handlers().await;
        info!("EventSubServiceV2: Registered {} event handlers", handlers.len());
        for handler in &handlers {
            debug!("  - {}: {:?} for {:?}", handler.id, handler.event_types, handler.platforms);
        }
        
        Ok(())
    }

    /// Spawn a task to listen to the event bus and dispatch to registered handlers
    pub async fn start(&self) {
        let mut rx = self.event_bus.subscribe(None).await;

        info!("EventSubServiceV2: Started, listening on EventBus");

        while let Some(event) = rx.recv().await {
            // Only process TwitchEventSub events
            if matches!(event, BotEvent::TwitchEventSub(_)) {
                debug!("EventSubServiceV2: Received event: {:?}", event);
                
                // Get all handlers that can process this event
                let handlers = self.registry.get_handlers_for_event(&event).await;
                
                if handlers.is_empty() {
                    debug!("EventSubServiceV2: No handlers registered for event type");
                } else {
                    debug!("EventSubServiceV2: Found {} handlers for event", handlers.len());
                    
                    // Execute handlers in priority order
                    for handler in handlers {
                        debug!("EventSubServiceV2: Executing handler '{}'", handler.id());
                        
                        match handler.handle(&event, &self.context).await {
                            Ok(true) => {
                                debug!("EventSubServiceV2: Handler '{}' processed event successfully", handler.id());
                            }
                            Ok(false) => {
                                debug!("EventSubServiceV2: Handler '{}' skipped event", handler.id());
                            }
                            Err(e) => {
                                error!("EventSubServiceV2: Handler '{}' failed: {:?}", handler.id(), e);
                            }
                        }
                    }
                }
            }
        }
        
        info!("EventSubServiceV2: Shutting down listener loop");
    }

    /// Add a custom handler at runtime
    pub async fn register_handler(&self, handler: Arc<dyn crate::services::event_handler::EventHandler>) -> Result<(), crate::Error> {
        info!("EventSubServiceV2: Registering custom handler '{}'", handler.id());
        self.registry.register(handler).await
    }

    /// Remove a handler by ID
    pub async fn unregister_handler(&self, handler_id: &str) -> Result<(), crate::Error> {
        info!("EventSubServiceV2: Unregistering handler '{}'", handler_id);
        self.registry.unregister(handler_id).await
    }

    /// List all registered handlers
    pub async fn list_handlers(&self) -> Vec<crate::services::event_registry::EventHandlerInfo> {
        self.registry.list_handlers().await
    }
}