use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use crate::Error;
use crate::eventbus::BotEvent;
use crate::services::event_context::EventContext;
use crate::services::event_pipeline::{EventFilter, FilterResult};

#[derive(Debug, Serialize, Deserialize)]
struct ChannelFilterConfig {
    channels: Vec<String>,
}

/// Filter by channel
pub struct ChannelFilter {
    channels: Vec<String>,
}

impl ChannelFilter {
    pub fn new(channels: Vec<String>) -> Self {
        Self { channels }
    }
}

#[async_trait]
impl EventFilter for ChannelFilter {
    fn id(&self) -> &str {
        "channel_filter"
    }

    fn name(&self) -> &str {
        "Channel Filter"
    }

    fn configure(&mut self, config: serde_json::Value) -> Result<(), Error> {
        let config: ChannelFilterConfig = serde_json::from_value(config)
            .map_err(|e| Error::Platform(format!("Invalid channel filter config: {}", e)))?;
        
        self.channels = config.channels;
        Ok(())
    }

    async fn apply(&self, event: &BotEvent, _context: &EventContext) -> Result<FilterResult, Error> {
        match event {
            BotEvent::ChatMessage { channel, .. } => {
                if self.channels.is_empty() || self.channels.iter().any(|c| c == channel) {
                    Ok(FilterResult::Pass)
                } else {
                    Ok(FilterResult::Reject)
                }
            }
            _ => Ok(FilterResult::Reject),
        }
    }
}