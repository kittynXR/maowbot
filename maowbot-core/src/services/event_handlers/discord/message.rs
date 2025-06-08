use async_trait::async_trait;
use tracing::{debug, info};
use maowbot_common::models::platform::Platform;

use crate::Error;
use crate::eventbus::BotEvent;
use crate::services::event_context::EventContext;
use crate::services::event_handler::{EventHandler, TypedEventHandler};

/// Handler for Discord message events
pub struct DiscordMessageHandler;

impl DiscordMessageHandler {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl EventHandler for DiscordMessageHandler {
    fn id(&self) -> &str {
        "discord.message"
    }

    fn event_types(&self) -> Vec<String> {
        vec!["message.create".to_string()]
    }

    fn platforms(&self) -> Vec<Platform> {
        vec![Platform::Discord]
    }

    async fn handle(&self, event: &BotEvent, ctx: &EventContext) -> Result<bool, Error> {
        match event {
            BotEvent::ChatMessage { platform, channel, user, text, timestamp, metadata: _ } => {
                if platform == "discord" {
                    // Process Discord message
                    debug!("DiscordMessageHandler: Processing message from {} in {}: {}", user, channel, text);
                    
                    // Message is already being processed by MessageService,
                    // but we can add Discord-specific handling here if needed
                    
                    // Example: Check for Discord-specific commands or mentions
                    if text.starts_with("!discord") {
                        info!("DiscordMessageHandler: Discord-specific command detected");
                        // Add custom Discord command handling
                    }
                    
                    Ok(true)
                } else {
                    Ok(false)
                }
            }
            _ => Ok(false),
        }
    }

    fn priority(&self) -> i32 {
        100 // Normal priority
    }
}

/// Handler for Discord message commands (non-slash commands)
pub struct DiscordCommandHandler {
    command_prefix: String,
}

impl DiscordCommandHandler {
    pub fn new(command_prefix: &str) -> Self {
        Self {
            command_prefix: command_prefix.to_string(),
        }
    }
}

#[async_trait]
impl EventHandler for DiscordCommandHandler {
    fn id(&self) -> &str {
        "discord.command"
    }

    fn event_types(&self) -> Vec<String> {
        vec!["message.create".to_string()]
    }

    fn platforms(&self) -> Vec<Platform> {
        vec![Platform::Discord]
    }

    async fn handle(&self, event: &BotEvent, ctx: &EventContext) -> Result<bool, Error> {
        match event {
            BotEvent::ChatMessage { platform, channel, user, text, .. } => {
                if platform == "discord" && text.starts_with(&self.command_prefix) {
                    debug!("DiscordCommandHandler: Processing command from {}: {}", user, text);
                    
                    // Extract command and args
                    let command_text = &text[self.command_prefix.len()..];
                    let parts: Vec<&str> = command_text.split_whitespace().collect();
                    
                    if parts.is_empty() {
                        return Ok(false);
                    }
                    
                    let command = parts[0];
                    let args = &parts[1..];
                    
                    // Handle Discord-specific commands
                    match command {
                        "help" => {
                            // Send help message
                            ctx.platform_manager
                                .send_discord_message("bot", "", channel, "Discord bot help: ...")
                                .await?;
                        }
                        "ping" => {
                            ctx.platform_manager
                                .send_discord_message("bot", "", channel, "Pong!")
                                .await?;
                        }
                        _ => {
                            debug!("Unknown Discord command: {}", command);
                        }
                    }
                    
                    Ok(true)
                } else {
                    Ok(false)
                }
            }
            _ => Ok(false),
        }
    }

    fn priority(&self) -> i32 {
        50 // Higher priority to process commands before general message handling
    }
}