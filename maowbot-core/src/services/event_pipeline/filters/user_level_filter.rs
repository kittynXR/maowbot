use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use crate::Error;
use crate::eventbus::BotEvent;
use crate::services::event_context::EventContext;
use crate::services::event_pipeline::{EventFilter, FilterResult};

#[derive(Debug, Serialize, Deserialize)]
struct UserLevelFilterConfig {
    #[serde(default = "default_min_level")]
    min_level: String,
    #[serde(default)]
    allowed_levels: Vec<String>,
}

fn default_min_level() -> String {
    "viewer".to_string()
}

/// Filter by user level (viewer, subscriber, vip, moderator, broadcaster)
pub struct UserLevelFilter {
    min_level: String,
    allowed_levels: Vec<String>,
}

impl UserLevelFilter {
    pub fn new(min_level: String) -> Self {
        Self {
            min_level,
            allowed_levels: vec![],
        }
    }
    
    fn level_to_numeric(&self, level: &str) -> u8 {
        match level.to_lowercase().as_str() {
            "viewer" => 0,
            "follower" => 1,
            "subscriber" => 2,
            "vip" => 3,
            "moderator" => 4,
            "broadcaster" | "owner" => 5,
            _ => 0,
        }
    }
}

#[async_trait]
impl EventFilter for UserLevelFilter {
    fn id(&self) -> &str {
        "user_level_filter"
    }

    fn name(&self) -> &str {
        "User Level Filter"
    }

    fn configure(&mut self, config: serde_json::Value) -> Result<(), Error> {
        let config: UserLevelFilterConfig = serde_json::from_value(config)
            .map_err(|e| Error::Platform(format!("Invalid user level filter config: {}", e)))?;
        
        self.min_level = config.min_level;
        self.allowed_levels = config.allowed_levels;
        Ok(())
    }

    async fn apply(&self, event: &BotEvent, _context: &EventContext) -> Result<FilterResult, Error> {
        match event {
            BotEvent::ChatMessage { metadata, .. } => {
                // Get user level from metadata
                let user_level = metadata.get("level")
                    .and_then(|l| l.as_str())
                    .unwrap_or("viewer");
                
                // Check allowed levels first
                if !self.allowed_levels.is_empty() {
                    if self.allowed_levels.iter().any(|l| l.eq_ignore_ascii_case(user_level)) {
                        return Ok(FilterResult::Pass);
                    } else {
                        return Ok(FilterResult::Reject);
                    }
                }
                
                // Check minimum level
                let user_level_num = self.level_to_numeric(user_level);
                let min_level_num = self.level_to_numeric(&self.min_level);
                
                if user_level_num >= min_level_num {
                    Ok(FilterResult::Pass)
                } else {
                    Ok(FilterResult::Reject)
                }
            }
            _ => Ok(FilterResult::Reject),
        }
    }
}