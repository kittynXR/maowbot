// Example demonstrating how to create custom event handlers

use async_trait::async_trait;
use tracing::info;
use maowbot_common::models::platform::Platform;
use crate::Error;
use crate::eventbus::BotEvent;
use crate::services::event_context::EventContext;
use crate::services::event_handler::EventHandler;

/// Example custom event handler that logs all chat messages
pub struct ChatLoggerHandler {
    log_prefix: String,
}

impl ChatLoggerHandler {
    pub fn new(log_prefix: &str) -> Self {
        Self {
            log_prefix: log_prefix.to_string(),
        }
    }
}

#[async_trait]
impl EventHandler for ChatLoggerHandler {
    fn id(&self) -> &str {
        "example.chat_logger"
    }

    fn event_types(&self) -> Vec<String> {
        vec!["chat.message".to_string()]
    }

    fn platforms(&self) -> Vec<Platform> {
        vec![Platform::Twitch, Platform::Discord, Platform::TwitchIRC]
    }

    async fn handle(&self, event: &BotEvent, _ctx: &EventContext) -> Result<bool, Error> {
        match event {
            BotEvent::ChatMessage { platform, channel, user, text, timestamp } => {
                info!(
                    "{} [{}] {}/{} <{}> {}",
                    self.log_prefix, timestamp, platform, channel, user, text
                );
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    fn priority(&self) -> i32 {
        200 // Lower priority, runs after other handlers
    }
}

/// Example of a composite handler that triggers multiple actions
pub struct StreamNotificationHandler {
    send_discord: bool,
    send_osc: bool,
}

impl StreamNotificationHandler {
    pub fn new(send_discord: bool, send_osc: bool) -> Self {
        Self { send_discord, send_osc }
    }
}

#[async_trait]
impl EventHandler for StreamNotificationHandler {
    fn id(&self) -> &str {
        "example.stream_notifications"
    }

    fn event_types(&self) -> Vec<String> {
        vec!["stream.online".to_string(), "stream.offline".to_string()]
    }

    fn platforms(&self) -> Vec<Platform> {
        vec![Platform::Twitch, Platform::TwitchEventSub]
    }

    async fn handle(&self, event: &BotEvent, ctx: &EventContext) -> Result<bool, Error> {
        // This shows how a single handler can perform multiple actions
        if self.send_discord {
            // Handler logic would check Discord configs and send notifications
            info!("Would send Discord notification for stream event");
        }
        
        if self.send_osc {
            // Handler logic would trigger OSC events
            info!("Would trigger OSC event for stream status");
        }
        
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use crate::services::event_registry::EventHandlerRegistry;

    #[tokio::test]
    async fn test_custom_handler_registration() {
        let registry = EventHandlerRegistry::new();
        
        // Create and register a custom handler
        let handler = Arc::new(ChatLoggerHandler::new("TEST"));
        assert!(registry.register(handler).await.is_ok());
        
        // Verify it's registered
        let handlers = registry.list_handlers().await;
        assert!(handlers.iter().any(|h| h.id == "example.chat_logger"));
    }
}