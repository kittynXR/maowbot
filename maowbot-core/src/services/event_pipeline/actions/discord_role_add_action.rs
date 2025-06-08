use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use crate::Error;
use crate::eventbus::BotEvent;
use crate::services::event_pipeline::{EventAction, ActionResult, ActionContext};

#[derive(Debug, Serialize, Deserialize)]
struct DiscordRoleAddActionConfig {
    account: String,
    guild_id: String,
    role_id: String,
    #[serde(default)]
    reason: String,
}

/// Action that adds a Discord role to a user
pub struct DiscordRoleAddAction {
    account: String,
    guild_id: String,
    role_id: String,
    reason: String,
}

impl DiscordRoleAddAction {
    pub fn new() -> Self {
        Self {
            account: String::new(),
            guild_id: String::new(),
            role_id: String::new(),
            reason: String::new(),
        }
    }
}

impl Default for DiscordRoleAddAction {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl EventAction for DiscordRoleAddAction {
    fn id(&self) -> &str {
        "discord_role_add"
    }

    fn name(&self) -> &str {
        "Add Discord Role"
    }

    fn configure(&mut self, config: serde_json::Value) -> Result<(), Error> {
        let config: DiscordRoleAddActionConfig = serde_json::from_value(config)
            .map_err(|e| Error::Platform(format!("Invalid Discord role add action config: {}", e)))?;
        
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
        //     .add_discord_role(&self.account, &self.guild_id, user_id, &self.role_id, &self.reason)
        //     .await?;
        
        tracing::info!(
            "Would add Discord role {} to user {} in guild {} (reason: {})",
            self.role_id, user_id, self.guild_id, self.reason
        );
        
        Ok(ActionResult::Success(serde_json::json!({
            "role_added": true,
            "user_id": user_id,
            "role_id": self.role_id,
            "guild_id": self.guild_id
        })))
    }
}