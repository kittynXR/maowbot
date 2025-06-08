use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use crate::Error;
use crate::eventbus::BotEvent;
use crate::services::event_context::EventContext;
use crate::services::event_pipeline::{EventFilter, FilterResult};
use maowbot_common::models::platform::Platform;

#[derive(Debug, Serialize, Deserialize)]
struct PlatformFilterConfig {
    platforms: Vec<String>,
}

/// Filter by platform
pub struct PlatformFilter {
    platforms: Vec<Platform>,
}

impl PlatformFilter {
    pub fn new(platforms: Vec<Platform>) -> Self {
        Self { platforms }
    }
}

#[async_trait]
impl EventFilter for PlatformFilter {
    fn id(&self) -> &str {
        "platform_filter"
    }

    fn name(&self) -> &str {
        "Platform Filter"
    }

    fn configure(&mut self, config: serde_json::Value) -> Result<(), Error> {
        let config: PlatformFilterConfig = serde_json::from_value(config)
            .map_err(|e| Error::Platform(format!("Invalid platform filter config: {}", e)))?;
        
        self.platforms = config.platforms
            .into_iter()
            .map(|s| Platform::from_string(&s))
            .collect();
        
        Ok(())
    }

    async fn apply(&self, event: &BotEvent, _context: &EventContext) -> Result<FilterResult, Error> {
        let platform = match event {
            BotEvent::ChatMessage { platform, .. } => Platform::from_string(platform),
            BotEvent::TwitchEventSub(_) => Platform::TwitchEventSub,
            _ => return Ok(FilterResult::Reject),
        };

        if self.platforms.is_empty() || self.platforms.contains(&platform) {
            Ok(FilterResult::Pass)
        } else {
            Ok(FilterResult::Reject)
        }
    }
}