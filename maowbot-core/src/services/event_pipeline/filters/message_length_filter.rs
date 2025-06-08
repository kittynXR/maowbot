use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use crate::Error;
use crate::eventbus::BotEvent;
use crate::services::event_context::EventContext;
use crate::services::event_pipeline::{EventFilter, FilterResult};

#[derive(Debug, Serialize, Deserialize)]
struct MessageLengthFilterConfig {
    #[serde(default)]
    min_length: usize,
    #[serde(default = "default_max_length")]
    max_length: usize,
}

fn default_max_length() -> usize {
    500
}

/// Filter by message length
pub struct MessageLengthFilter {
    min_length: usize,
    max_length: usize,
}

impl MessageLengthFilter {
    pub fn new(min_length: usize, max_length: usize) -> Self {
        Self {
            min_length,
            max_length,
        }
    }
}

#[async_trait]
impl EventFilter for MessageLengthFilter {
    fn id(&self) -> &str {
        "message_length_filter"
    }

    fn name(&self) -> &str {
        "Message Length Filter"
    }

    fn configure(&mut self, config: serde_json::Value) -> Result<(), Error> {
        let config: MessageLengthFilterConfig = serde_json::from_value(config)
            .map_err(|e| Error::Platform(format!("Invalid message length filter config: {}", e)))?;
        
        self.min_length = config.min_length;
        self.max_length = config.max_length;
        Ok(())
    }

    async fn apply(&self, event: &BotEvent, _context: &EventContext) -> Result<FilterResult, Error> {
        match event {
            BotEvent::ChatMessage { text, .. } => {
                let length = text.chars().count();
                
                if length >= self.min_length && length <= self.max_length {
                    Ok(FilterResult::Pass)
                } else {
                    Ok(FilterResult::Reject)
                }
            }
            _ => Ok(FilterResult::Reject),
        }
    }
}