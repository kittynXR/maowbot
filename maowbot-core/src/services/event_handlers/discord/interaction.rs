use async_trait::async_trait;
use tracing::{debug, info, error};
use maowbot_common::models::platform::Platform;

use crate::Error;
use crate::eventbus::BotEvent;
use crate::services::event_context::EventContext;
use crate::services::event_handler::EventHandler;

/// Handler for Discord interaction events (slash commands, buttons, etc.)
pub struct InteractionHandler;

impl InteractionHandler {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl EventHandler for InteractionHandler {
    fn id(&self) -> &str {
        "discord.interaction"
    }

    fn event_types(&self) -> Vec<String> {
        vec!["interaction.create".to_string()]
    }

    fn platforms(&self) -> Vec<Platform> {
        vec![Platform::Discord]
    }

    async fn handle(&self, event: &BotEvent, ctx: &EventContext) -> Result<bool, Error> {
        // Note: Currently, slash commands are handled directly in the Discord runtime
        // via the slashcommands module. This handler is a placeholder for when we
        // create a proper DiscordEvent enum in the BotEvent system.
        
        debug!("InteractionHandler: Would handle interaction event");
        Ok(false)
    }

    fn priority(&self) -> i32 {
        50 // Higher priority for interactive commands
    }
}

/// Handler for specific slash commands
pub struct SlashCommandHandler {
    command_name: String,
}

impl SlashCommandHandler {
    pub fn new(command_name: &str) -> Self {
        Self {
            command_name: command_name.to_string(),
        }
    }
    
    pub async fn handle_command(
        &self,
        ctx: &EventContext,
        guild_id: Option<&str>,
        channel_id: &str,
        user_id: &str,
        options: &[(&str, &str)],
    ) -> Result<(), Error> {
        match self.command_name.as_str() {
            "ping" => {
                info!("SlashCommandHandler: Handling /ping command from user {}", user_id);
                // Response would be sent through interaction response API
                // which is different from regular messages
            }
            "help" => {
                info!("SlashCommandHandler: Handling /help command from user {}", user_id);
                // Send help embed
            }
            "config" => {
                if guild_id.is_none() {
                    debug!("Config command used outside of guild context");
                    return Ok(());
                }
                
                info!("SlashCommandHandler: Handling /config command in guild {:?}", guild_id);
                // Handle configuration commands
            }
            _ => {
                debug!("Unknown slash command: {}", self.command_name);
            }
        }
        
        Ok(())
    }
}

#[async_trait]
impl EventHandler for SlashCommandHandler {
    fn id(&self) -> &str {
        "discord.slash_command"
    }

    fn event_types(&self) -> Vec<String> {
        vec!["interaction.create".to_string()]
    }

    fn platforms(&self) -> Vec<Platform> {
        vec![Platform::Discord]
    }

    async fn handle(&self, event: &BotEvent, ctx: &EventContext) -> Result<bool, Error> {
        // This would handle specific slash commands when we add them to BotEvent
        debug!("SlashCommandHandler: Would handle slash command: {}", self.command_name);
        Ok(false)
    }

    fn priority(&self) -> i32 {
        40 // Higher priority for specific command handling
    }
}