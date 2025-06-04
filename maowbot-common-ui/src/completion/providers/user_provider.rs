// Completion provider for usernames from chat cache
use crate::completion::{CompletionProvider, CompletionItem, CompletionCategory, CompletionContext};
use crate::GrpcClient;
use async_trait::async_trait;
use std::sync::Arc;
use std::collections::HashSet;

pub struct UserCompletionProvider {
    client: Arc<GrpcClient>,
}

impl UserCompletionProvider {
    pub fn new(client: Arc<GrpcClient>) -> Self {
        Self { client }
    }
    
    async fn get_recent_chatters(&self, channel: &str, limit: usize) -> Vec<(String, Vec<String>)> {
        // TODO: This should query the message cache service
        // For now, we'll use a placeholder that would be replaced with actual gRPC call
        
        // In real implementation:
        // - Query message cache for recent messages in channel
        // - Extract unique usernames
        // - Include their roles/badges
        
        vec![]
    }
}

#[async_trait]
impl CompletionProvider for UserCompletionProvider {
    fn name(&self) -> &str {
        "users"
    }
    
    fn is_applicable(&self, context: &CompletionContext) -> bool {
        // Complete usernames when:
        // 1. Typing an @mention
        // 2. In TUI for user commands
        context.is_mention() || 
        (matches!(&context.scope, crate::completion::CompletionScope::TuiCommand) && 
         context.previous_words().get(0) == Some(&"user"))
    }
    
    async fn provide_completions(
        &self,
        context: &CompletionContext,
        prefix: &str,
    ) -> Result<Vec<CompletionItem>, Box<dyn std::error::Error + Send + Sync>> {
        let mut items = Vec::new();
        
        // For @mentions in chat
        if context.is_mention() {
            let search_prefix = prefix.strip_prefix('@').unwrap_or(prefix).to_lowercase();
            let channel = context.channel().unwrap_or_default();
            
            // Get recent chatters
            let chatters = self.get_recent_chatters(channel, 100).await;
            
            for (username, roles) in chatters {
                if username.to_lowercase().starts_with(&search_prefix) {
                    items.push(CompletionItem {
                        replacement: format!("@{}", username),
                        display: format!("@{}", username),
                        description: Some(roles.join(", ")),
                        category: CompletionCategory::Username,
                        icon: Some("@".to_string()),
                        priority: 90,
                        metadata: [("roles".to_string(), roles.join(","))].into_iter().collect(),
                    });
                }
            }
        }
        
        // For TUI user commands
        if matches!(&context.scope, crate::completion::CompletionScope::TuiCommand) {
            use maowbot_proto::maowbot::services::{ListUsersRequest, ListUsersFilter};
            use maowbot_proto::maowbot::common::PageRequest;
            
            let request = ListUsersRequest {
                page: Some(PageRequest {
                    page_size: 50,
                    page_token: String::new(),
                }),
                filter: Some(ListUsersFilter {
                    active_only: true,
                    platforms: vec![],
                    roles: vec![],
                }),
                order_by: "last_seen".to_string(),
                descending: true,
            };
            
            if let Ok(response) = self.client.user.clone().list_users(request).await {
                for user in response.into_inner().users {
                    if user.global_username.to_lowercase().starts_with(&prefix.to_lowercase()) {
                        items.push(CompletionItem {
                            replacement: user.global_username.clone(),
                            display: user.global_username.clone(),
                            description: Some(format!("User ID: {}", &user.user_id[..8])),
                            category: CompletionCategory::Username,
                            icon: Some("👤".to_string()),
                            priority: 85,
                            metadata: [("user_id".to_string(), user.user_id)].into_iter().collect(),
                        });
                    }
                }
            }
        }
        
        // Remove duplicates and sort
        let mut seen = HashSet::new();
        items.retain(|item| seen.insert(item.replacement.clone()));
        items.sort_by(|a, b| {
            b.priority.cmp(&a.priority)
                .then_with(|| a.replacement.len().cmp(&b.replacement.len()))
        });
        
        Ok(items)
    }
    
    fn cache_duration(&self) -> std::time::Duration {
        // Cache for 5 minutes
        std::time::Duration::from_secs(300)
    }
}