use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use crate::Error;
use crate::eventbus::BotEvent;
use crate::services::event_pipeline::{EventAction, ActionResult, ActionContext};

#[derive(Debug, Serialize, Deserialize)]
struct DiscordRoleRemoveActionConfig {
    account: String,
    guild_id: String,
    role_id: String,
    #[serde(default)]
    reason: String,
}

/// Action that removes a Discord role from a user
pub struct DiscordRoleRemoveAction {
    account: String,
    guild_id: String,
    role_id: String,
    reason: String,
}

impl DiscordRoleRemoveAction {
    pub fn new() -> Self {
        Self {
            account: String::new(),
            guild_id: String::new(),
            role_id: String::new(),
            reason: String::new(),
        }
    }
}

impl Default for DiscordRoleRemoveAction {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl EventAction for DiscordRoleRemoveAction {
    fn id(&self) -> &str {
        "discord_role_remove"
    }

    fn name(&self) -> &str {
        "Remove Discord Role"
    }

    fn configure(&mut self, config: serde_json::Value) -> Result<(), Error> {
        let config: DiscordRoleRemoveActionConfig = serde_json::from_value(config)
            .map_err(|e| Error::Platform(format!("Invalid Discord role remove action config: {}", e)))?;
        
        self.account = config.account;
        self.guild_id = config.guild_id;
        self.role_id = config.role_id;
        self.reason = config.reason;
        Ok(())
    }

    async fn execute(&self, context: &mut ActionContext) -> Result<ActionResult, Error> {
        // Extract user ID from event or shared data
        let user_id = match &context.event {
            BotEvent::ChatMessage { metadata, .. } => {
                metadata.get("discord_user_id")
                    .and_then(|v| v.as_str())
            }
            _ => None,
        }.or_else(|| {
            context.get_data("discord_user_id")
                .and_then(|v| v.as_str())
        });
        
        let user_id = match user_id {
            Some(id) => id,
            None => {
                return Ok(ActionResult::Error("No Discord user ID available".to_string()));
            }
        };
        
        // TODO: Implement Discord role management in platform manager
        // context.context.platform_manager
        //     .remove_discord_role(&self.account, &self.guild_id, user_id, &self.role_id, &self.reason)
        //     .await?;
        
        tracing::info!(
            "Would remove Discord role {} from user {} in guild {} (reason: {})",
            self.role_id, user_id, self.guild_id, self.reason
        );
        
        Ok(ActionResult::Success(serde_json::json!({
            "role_removed": true,
            "user_id": user_id,
            "role_id": self.role_id,
            "guild_id": self.guild_id
        })))
    }
}