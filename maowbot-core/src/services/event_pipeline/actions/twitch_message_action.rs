use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use crate::Error;
use crate::eventbus::BotEvent;
use crate::services::event_pipeline::{EventAction, ActionResult, ActionContext};

#[derive(Debug, Serialize, Deserialize)]
struct TwitchMessageActionConfig {
    account: String,
    #[serde(default)]
    channel: String,
    message_template: String,
    #[serde(default)]
    reply_to_message: bool,
}

/// Action that sends a Twitch chat message
pub struct TwitchMessageAction {
    account: String,
    channel: String,
    message_template: String,
    reply_to_message: bool,
}

impl TwitchMessageAction {
    pub fn new() -> Self {
        Self {
            account: String::new(),
            channel: String::new(),
            message_template: String::new(),
            reply_to_message: false,
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

impl Default for TwitchMessageAction {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl EventAction for TwitchMessageAction {
    fn id(&self) -> &str {
        "twitch_message"
    }

    fn name(&self) -> &str {
        "Send Twitch Message"
    }

    fn configure(&mut self, config: serde_json::Value) -> Result<(), Error> {
        let config: TwitchMessageActionConfig = serde_json::from_value(config)
            .map_err(|e| Error::Platform(format!("Invalid Twitch message action config: {}", e)))?;
        
        self.account = config.account;
        self.channel = config.channel;
        self.message_template = config.message_template;
        self.reply_to_message = config.reply_to_message;
        Ok(())
    }

    async fn execute(&self, context: &mut ActionContext) -> Result<ActionResult, Error> {
        let message = self.format_message(context);
        
        // Get channel from config or event
        let channel = if !self.channel.is_empty() {
            self.channel.clone()
        } else {
            match &context.event {
                BotEvent::ChatMessage { channel, .. } => channel.clone(),
                _ => {
                    return Ok(ActionResult::Error("No channel specified".to_string()));
                }
            }
        };
        
        // Get message ID if we need to reply
        let reply_to_id = if self.reply_to_message {
            match &context.event {
                BotEvent::ChatMessage { metadata, .. } => {
                    metadata.get("message_id").and_then(|v| v.as_str())
                }
                _ => None,
            }
        } else {
            None
        };
        
        // Send message through Twitch
        // TODO: Get proper user ID from context
        let user_id = uuid::Uuid::new_v4();
        context.context.message_sender
            .send_twitch_message(
                &channel,
                &message,
                None, // credential_id
                user_id,
            )
            .await?;
        
        Ok(ActionResult::Success(serde_json::json!({
            "message_sent": true,
            "channel": channel,
            "replied_to": reply_to_id,
            "message_length": message.len()
        })))
    }
}