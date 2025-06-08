use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use crate::Error;
use crate::eventbus::BotEvent;
use crate::services::event_pipeline::{EventAction, ActionResult, ActionContext};

#[derive(Debug, Serialize, Deserialize)]
struct DiscordMessageActionConfig {
    account: String,
    #[serde(default)]
    guild_id: String,
    channel_id: String,
    message_template: String,
}

/// Action that sends a Discord message
pub struct DiscordMessageAction {
    account: String,
    guild_id: String,
    channel_id: String,
    message_template: String,
}

impl DiscordMessageAction {
    pub fn new() -> Self {
        Self {
            account: String::new(),
            guild_id: String::new(),
            channel_id: String::new(),
            message_template: String::new(),
        }
    }
    
    fn format_message(&self, context: &ActionContext) -> String {
        let mut message = self.message_template.clone();
        
        // Replace common placeholders
        match &context.event {
            BotEvent::ChatMessage { platform, channel, user, text, .. } => {
                message = message.replace("{platform}", platform);
                message = message.replace("{channel}", channel);
                message = message.replace("{user}", user);
                message = message.replace("{message}", text);
                message = message.replace("{text}", text);
            }
            BotEvent::TwitchEventSub(event) => {
                message = message.replace("{event_type}", &format!("{:?}", event));
            }
            _ => {}
        }
        
        // Replace shared data placeholders
        for (key, value) in &context.shared_data {
            if let Some(str_val) = value.as_str() {
                message = message.replace(&format!("{{{}}}", key), str_val);
            }
        }
        
        message
    }
}

impl Default for DiscordMessageAction {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl EventAction for DiscordMessageAction {
    fn id(&self) -> &str {
        "discord_message"
    }

    fn name(&self) -> &str {
        "Send Discord Message"
    }

    fn configure(&mut self, config: serde_json::Value) -> Result<(), Error> {
        let config: DiscordMessageActionConfig = serde_json::from_value(config)
            .map_err(|e| Error::Platform(format!("Invalid Discord message action config: {}", e)))?;
        
        self.account = config.account;
        self.guild_id = config.guild_id;
        self.channel_id = config.channel_id;
        self.message_template = config.message_template;
        Ok(())
    }

    async fn execute(&self, context: &mut ActionContext) -> Result<ActionResult, Error> {
        let message = self.format_message(context);
        
        // Use guild_id from config or from shared data
        let guild_id = if !self.guild_id.is_empty() {
            &self.guild_id
        } else {
            context.get_data("guild_id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
        };
        
        context.context.platform_manager
            .send_discord_message(&self.account, guild_id, &self.channel_id, &message)
            .await?;
        
        Ok(ActionResult::Success(serde_json::json!({
            "message_sent": true,
            "channel_id": self.channel_id,
            "message_length": message.len()
        })))
    }
}