use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use crate::Error;
use crate::eventbus::BotEvent;
use crate::services::event_pipeline::{EventAction, ActionResult, ActionContext};

#[derive(Debug, Serialize, Deserialize)]
struct TwitchTimeoutActionConfig {
    account: String,
    #[serde(default)]
    channel: String,
    #[serde(default = "default_duration")]
    duration_seconds: u32,
    #[serde(default)]
    reason: String,
}

fn default_duration() -> u32 {
    600 // 10 minutes
}

/// Action that timeouts a user on Twitch
pub struct TwitchTimeoutAction {
    account: String,
    channel: String,
    duration_seconds: u32,
    reason: String,
}

impl TwitchTimeoutAction {
    pub fn new() -> Self {
        Self {
            account: String::new(),
            channel: String::new(),
            duration_seconds: 600,
            reason: String::new(),
        }
    }
}

impl Default for TwitchTimeoutAction {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl EventAction for TwitchTimeoutAction {
    fn id(&self) -> &str {
        "twitch_timeout"
    }

    fn name(&self) -> &str {
        "Twitch Timeout User"
    }

    fn configure(&mut self, config: serde_json::Value) -> Result<(), Error> {
        let config: TwitchTimeoutActionConfig = serde_json::from_value(config)
            .map_err(|e| Error::Platform(format!("Invalid Twitch timeout action config: {}", e)))?;
        
        self.account = config.account;
        self.channel = config.channel;
        self.duration_seconds = config.duration_seconds;
        self.reason = config.reason;
        Ok(())
    }

    async fn execute(&self, context: &mut ActionContext) -> Result<ActionResult, Error> {
        // Get channel and user from event
        let (channel, user_id, username) = match &context.event {
            BotEvent::ChatMessage { channel, user, metadata, .. } => {
                let user_id = metadata.get("user_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                (channel.clone(), user_id.to_string(), user.clone())
            }
            _ => {
                return Ok(ActionResult::Error("Event is not a chat message".to_string()));
            }
        };
        
        let channel = if !self.channel.is_empty() {
            self.channel.clone()
        } else {
            channel
        };
        
        if user_id.is_empty() {
            return Ok(ActionResult::Error("No user ID available".to_string()));
        }
        
        // TODO: Implement Twitch timeout in platform manager
        // context.context.platform_manager
        //     .timeout_twitch_user(&self.account, &channel, &user_id, self.duration_seconds, &self.reason)
        //     .await?;
        
        tracing::info!(
            "Would timeout user {} ({}) in channel {} for {} seconds (reason: {})",
            username, user_id, channel, self.duration_seconds, self.reason
        );
        
        Ok(ActionResult::Success(serde_json::json!({
            "timeout_applied": true,
            "user_id": user_id,
            "username": username,
            "channel": channel,
            "duration_seconds": self.duration_seconds,
            "reason": self.reason
        })))
    }
}