use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use regex::Regex;
use crate::Error;
use crate::eventbus::BotEvent;
use crate::services::event_context::EventContext;
use crate::services::event_pipeline::{EventFilter, FilterResult};

#[derive(Debug, Serialize, Deserialize)]
struct MessagePatternFilterConfig {
    patterns: Vec<String>,
    #[serde(default = "default_match_any")]
    match_any: bool,
    #[serde(default)]
    case_insensitive: bool,
}

fn default_match_any() -> bool {
    true
}

/// Filter by message content pattern
pub struct MessagePatternFilter {
    patterns: Vec<Regex>,
    match_any: bool,
}

impl MessagePatternFilter {
    pub fn new(patterns: Vec<&str>, match_any: bool) -> Result<Self, Error> {
        let compiled_patterns = patterns
            .into_iter()
            .map(|p| Regex::new(p))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| Error::Platform(format!("Invalid regex pattern: {}", e)))?;
        
        Ok(Self {
            patterns: compiled_patterns,
            match_any,
        })
    }
}

#[async_trait]
impl EventFilter for MessagePatternFilter {
    fn id(&self) -> &str {
        "message_pattern_filter"
    }

    fn name(&self) -> &str {
        "Message Pattern Filter"
    }

    fn configure(&mut self, config: serde_json::Value) -> Result<(), Error> {
        let config: MessagePatternFilterConfig = serde_json::from_value(config)
            .map_err(|e| Error::Platform(format!("Invalid message pattern filter config: {}", e)))?;
        
        self.patterns = config.patterns
            .into_iter()
            .map(|p| {
                if config.case_insensitive {
                    Regex::new(&format!("(?i){}", p))
                } else {
                    Regex::new(&p)
                }
            })
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| Error::Platform(format!("Invalid regex pattern: {}", e)))?;
        
        self.match_any = config.match_any;
        Ok(())
    }

    async fn apply(&self, event: &BotEvent, _context: &EventContext) -> Result<FilterResult, Error> {
        match event {
            BotEvent::ChatMessage { text, .. } => {
                if self.patterns.is_empty() {
                    return Ok(FilterResult::Pass);
                }
                
                let matches = self.patterns.iter().filter(|p| p.is_match(text)).count();
                
                let result = if self.match_any {
                    matches > 0
                } else {
                    matches == self.patterns.len()
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