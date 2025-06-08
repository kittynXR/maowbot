use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use crate::Error;
use crate::eventbus::BotEvent;
use crate::services::event_context::EventContext;
use crate::services::event_pipeline::{EventFilter, FilterResult};

#[derive(Debug, Serialize, Deserialize)]
struct UserRoleFilterConfig {
    required_roles: Vec<String>,
    #[serde(default = "default_match_any")]
    match_any: bool,
}

fn default_match_any() -> bool {
    true
}

/// Filter by user roles
pub struct UserRoleFilter {
    required_roles: Vec<String>,
    match_any: bool, // true = OR, false = AND
}

impl UserRoleFilter {
    pub fn new(required_roles: Vec<String>, match_any: bool) -> Self {
        Self {
            required_roles,
            match_any,
        }
    }
}

#[async_trait]
impl EventFilter for UserRoleFilter {
    fn id(&self) -> &str {
        "user_role_filter"
    }

    fn name(&self) -> &str {
        "User Role Filter"
    }

    fn configure(&mut self, config: serde_json::Value) -> Result<(), Error> {
        let config: UserRoleFilterConfig = serde_json::from_value(config)
            .map_err(|e| Error::Platform(format!("Invalid user role filter config: {}", e)))?;
        
        self.required_roles = config.required_roles;
        self.match_any = config.match_any;
        Ok(())
    }

    async fn apply(&self, event: &BotEvent, context: &EventContext) -> Result<FilterResult, Error> {
        match event {
            BotEvent::ChatMessage { platform, user, .. } => {
                // Get user from database
                let user_record = context.user_service
                    .get_or_create_user(platform, user, None)
                    .await?;
                
                // TODO: Implement role checking once role system is in place
                // For now, check against message metadata roles if available
                let roles = match event {
                    BotEvent::ChatMessage { metadata, .. } => {
                        metadata.get("roles")
                            .and_then(|r| r.as_array())
                            .map(|arr| arr.iter()
                                .filter_map(|v| v.as_str())
                                .map(String::from)
                                .collect::<Vec<_>>())
                            .unwrap_or_default()
                    }
                    _ => vec![],
                };
                
                if self.required_roles.is_empty() {
                    return Ok(FilterResult::Pass);
                }
                
                let matches = self.required_roles.iter()
                    .filter(|required| roles.contains(required))
                    .count();
                
                let result = if self.match_any {
                    matches > 0
                } else {
                    matches == self.required_roles.len()
                };
                
                if result {
                    Ok(FilterResult::Pass)
                } else {
                    Ok(FilterResult::Reject)
                }
            }
            _ => Ok(FilterResult::Reject),
        }
    }
}