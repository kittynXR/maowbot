use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, debug, warn};
use async_trait::async_trait;

use crate::Error;
use crate::eventbus::BotEvent;
use crate::services::event_handler::{EventHandler, EventHandlerInfo, EventHandlerExt};
use maowbot_common::models::platform::Platform;

/// Registry for dynamic event handler registration and management.
/// Handlers are organized by platform and event type for efficient lookup.
pub struct EventHandlerRegistry {
    /// Map of (platform, event_type) -> handlers sorted by priority
    handlers: Arc<RwLock<HashMap<(Platform, String), Vec<Arc<dyn EventHandler>>>>>,
    /// Map of handler ID -> handler for direct lookup
    handlers_by_id: Arc<RwLock<HashMap<String, Arc<dyn EventHandler>>>>,
}

impl EventHandlerRegistry {
    pub fn new() -> Self {
        Self {
            handlers: Arc::new(RwLock::new(HashMap::new())),
            handlers_by_id: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register a new event handler
    pub async fn register(&self, handler: Arc<dyn EventHandler>) -> Result<(), Error> {
        let handler_id = handler.id().to_string();
        let platforms = handler.platforms();
        let event_types = handler.event_types();
        let priority = handler.priority();

        info!(
            "Registering event handler '{}' for platforms {:?} and events {:?} with priority {}",
            handler_id, platforms, event_types, priority
        );

        // Add to ID map
        {
            let mut id_map = self.handlers_by_id.write().await;
            if id_map.contains_key(&handler_id) {
                return Err(Error::Platform(format!(
                    "Handler with ID '{}' already registered",
                    handler_id
                )));
            }
            id_map.insert(handler_id.clone(), handler.clone());
        }

        // Add to platform/event type map
        {
            let mut handlers_map = self.handlers.write().await;
            
            for platform in &platforms {
                for event_type in &event_types {
                    let key = (platform.clone(), event_type.clone());
                    let handler_list = handlers_map.entry(key).or_insert_with(Vec::new);
                    
                    // Insert in priority order (lower priority numbers first)
                    let insert_pos = handler_list
                        .binary_search_by_key(&priority, |h| h.priority())
                        .unwrap_or_else(|pos| pos);
                    
                    handler_list.insert(insert_pos, handler.clone());
                    
                    debug!(
                        "Handler '{}' registered for {:?}/{} at position {}",
                        handler_id, platform, event_type, insert_pos
                    );
                }
            }
        }

        Ok(())
    }

    /// Unregister a handler by ID
    pub async fn unregister(&self, handler_id: &str) -> Result<(), Error> {
        info!("Unregistering event handler '{}'", handler_id);

        // Remove from ID map and get the handler
        let handler = {
            let mut id_map = self.handlers_by_id.write().await;
            id_map.remove(handler_id).ok_or_else(|| {
                Error::Platform(format!("Handler '{}' not found", handler_id))
            })?
        };

        // Remove from platform/event type map
        {
            let mut handlers_map = self.handlers.write().await;
            let platforms = handler.platforms();
            let event_types = handler.event_types();

            for platform in &platforms {
                for event_type in &event_types {
                    let key = (platform.clone(), event_type.clone());
                    if let Some(handler_list) = handlers_map.get_mut(&key) {
                        handler_list.retain(|h| h.id() != handler_id);
                        if handler_list.is_empty() {
                            handlers_map.remove(&key);
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Get all handlers for a specific platform and event type
    pub async fn get_handlers(
        &self,
        platform: Platform,
        event_type: &str,
    ) -> Vec<Arc<dyn EventHandler>> {
        let handlers = self.handlers.read().await;
        let key = (platform, event_type.to_string());
        
        handlers
            .get(&key)
            .map(|h| h.clone())
            .unwrap_or_default()
            .into_iter()
            .filter(|h| h.is_enabled())
            .collect()
    }

    /// Get all handlers that can process a specific event
    pub async fn get_handlers_for_event(&self, event: &BotEvent) -> Vec<Arc<dyn EventHandler>> {
        let all_handlers = self.handlers_by_id.read().await;
        
        let mut matching_handlers: Vec<Arc<dyn EventHandler>> = all_handlers
            .values()
            .filter(|h| h.is_enabled() && h.can_handle(event))
            .cloned()
            .collect();
        
        // Sort by priority
        matching_handlers.sort_by_key(|h| h.priority());
        
        matching_handlers
    }

    /// Get handler by ID
    pub async fn get_handler(&self, handler_id: &str) -> Option<Arc<dyn EventHandler>> {
        let handlers = self.handlers_by_id.read().await;
        handlers.get(handler_id).cloned()
    }

    /// List all registered handlers
    pub async fn list_handlers(&self) -> Vec<EventHandlerInfo> {
        let handlers = self.handlers_by_id.read().await;
        
        handlers
            .values()
            .map(|h| EventHandlerInfo {
                id: h.id().to_string(),
                name: h.id().to_string(), // Could be expanded with a name() method
                description: String::new(), // Could be expanded with a description() method
                platforms: h.platforms(),
                event_types: h.event_types(),
                priority: h.priority(),
                enabled: h.is_enabled(),
            })
            .collect()
    }

    /// Enable or disable a handler
    pub async fn set_handler_enabled(&self, handler_id: &str, enabled: bool) -> Result<(), Error> {
        // This would require adding a set_enabled method to EventHandler trait
        // For now, we'll log a warning
        warn!(
            "set_handler_enabled not implemented yet (handler: {}, enabled: {})",
            handler_id, enabled
        );
        Ok(())
    }

    /// Clear all registered handlers
    pub async fn clear(&self) {
        let mut handlers = self.handlers.write().await;
        let mut handlers_by_id = self.handlers_by_id.write().await;
        
        handlers.clear();
        handlers_by_id.clear();
        
        info!("Cleared all event handlers from registry");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::event_context::EventContext;

    struct TestHandler {
        id: String,
        platforms: Vec<Platform>,
        event_types: Vec<String>,
        priority: i32,
    }

    #[async_trait]
    impl EventHandler for TestHandler {
        fn id(&self) -> &str {
            &self.id
        }

        fn event_types(&self) -> Vec<String> {
            self.event_types.clone()
        }

        fn platforms(&self) -> Vec<Platform> {
            self.platforms.clone()
        }

        async fn handle(&self, _event: &BotEvent, _ctx: &EventContext) -> Result<bool, Error> {
            Ok(true)
        }

        fn priority(&self) -> i32 {
            self.priority
        }
    }

    #[tokio::test]
    async fn test_handler_registration() {
        let registry = EventHandlerRegistry::new();
        
        let handler = Arc::new(TestHandler {
            id: "test_handler".to_string(),
            platforms: vec![Platform::Twitch],
            event_types: vec!["stream.online".to_string()],
            priority: 100,
        });

        // Register handler
        assert!(registry.register(handler.clone()).await.is_ok());
        
        // Should fail on duplicate registration
        assert!(registry.register(handler).await.is_err());
        
        // Should be able to get handler
        let handlers = registry.get_handlers(Platform::Twitch, "stream.online").await;
        assert_eq!(handlers.len(), 1);
        assert_eq!(handlers[0].id(), "test_handler");
    }

    #[tokio::test]
    async fn test_priority_ordering() {
        let registry = EventHandlerRegistry::new();
        
        // Register handlers with different priorities
        for (i, priority) in [(1, 200), (2, 100), (3, 150)].iter() {
            let handler = Arc::new(TestHandler {
                id: format!("handler_{}", i),
                platforms: vec![Platform::Twitch],
                event_types: vec!["test.event".to_string()],
                priority: *priority,
            });
            registry.register(handler).await.unwrap();
        }

        // Get handlers - should be sorted by priority
        let handlers = registry.get_handlers(Platform::Twitch, "test.event").await;
        assert_eq!(handlers.len(), 3);
        assert_eq!(handlers[0].id(), "handler_2"); // priority 100
        assert_eq!(handlers[1].id(), "handler_3"); // priority 150
        assert_eq!(handlers[2].id(), "handler_1"); // priority 200
    }
}