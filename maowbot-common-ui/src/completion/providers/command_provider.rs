// Completion provider for Twitch/Discord commands
use crate::completion::{CompletionProvider, CompletionItem, CompletionCategory, CompletionContext, CompletionScope};
use crate::GrpcClient;
use async_trait::async_trait;
use std::sync::Arc;

pub struct CommandCompletionProvider {
    client: Arc<GrpcClient>,
}

impl CommandCompletionProvider {
    pub fn new(client: Arc<GrpcClient>) -> Self {
        Self { client }
    }
}

#[async_trait]
impl CompletionProvider for CommandCompletionProvider {
    fn name(&self) -> &str {
        "commands"
    }
    
    fn is_applicable(&self, context: &CompletionContext) -> bool {
        // Only for chat contexts when typing a command
        matches!(
            &context.scope,
            CompletionScope::TwitchChat { .. } | 
            CompletionScope::DiscordChat { .. } |
            CompletionScope::OverlayChat { .. }
        ) && context.is_command()
    }
    
    async fn provide_completions(
        &self,
        context: &CompletionContext,
        prefix: &str,
    ) -> Result<Vec<CompletionItem>, Box<dyn std::error::Error + Send + Sync>> {
        use maowbot_proto::maowbot::services::ListCommandsRequest;
        
        let platform = context.platform().unwrap_or("twitch-irc");
        
        // Strip the ! from the prefix if present
        let command_prefix = prefix.strip_prefix('!').unwrap_or(prefix);
        
        let request = ListCommandsRequest {
            platform: platform.to_string(),
            active_only: true,
            name_prefix: command_prefix.to_string(),
            page: None,
        };
        
        let response = self.client.command.clone()
            .list_commands(request)
            .await?;
        
        let mut items = Vec::new();
        
        for cmd_info in response.into_inner().commands {
            if let Some(cmd) = cmd_info.command {
                // Check if user has permission
                let can_use = Self::check_permission(&cmd, &context.user_roles, context.is_stream_online);
                
                if can_use {
                    items.push(CompletionItem {
                        replacement: format!("!{}", cmd.name),
                        display: format!("!{}", cmd.name),
                        description: Some(cmd.description.chars().take(50).collect()),
                        category: CompletionCategory::Command,
                        icon: Some("!".to_string()),
                        priority: 100,
                        metadata: [
                            ("cooldown".to_string(), cmd.cooldown_seconds.to_string()),
                            ("platform".to_string(), cmd.platform),
                        ].into_iter().collect(),
                    });
                }
            }
        }
        
        Ok(items)
    }
}

impl CommandCompletionProvider {
    fn check_permission(
        cmd: &maowbot_proto::maowbot::common::Command,
        user_roles: &[String],
        _is_online: bool,
    ) -> bool {
        // If no required roles, everyone can use it
        if cmd.required_roles.is_empty() {
            return true;
        }
        
        // Check if user has any of the required roles
        user_roles.iter().any(|role| cmd.required_roles.contains(role))
    }
}