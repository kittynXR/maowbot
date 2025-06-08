use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use chrono::{DateTime, Utc, Duration};
use crate::Error;
use crate::eventbus::BotEvent;
use crate::services::event_context::EventContext;
use crate::services::event_pipeline::{EventFilter, FilterResult};

#[derive(Debug, Serialize, Deserialize)]
struct CooldownFilterConfig {
    #[serde(default = "default_cooldown_seconds")]
    cooldown_seconds: i64,
    #[serde(default = "default_per_user")]
    per_user: bool,
    #[serde(default)]
    per_channel: bool,
}

fn default_cooldown_seconds() -> i64 {
    60
}

fn default_per_user() -> bool {
    true
}

/// Filter that implements cooldown between executions
pub struct CooldownFilter {
    cooldown_seconds: i64,
    per_user: bool,
    per_channel: bool,
    last_execution: Arc<RwLock<HashMap<String, DateTime<Utc>>>>,
}

impl CooldownFilter {
    pub fn new(cooldown_seconds: i64, per_user: bool) -> Self {
        Self {
            cooldown_seconds,
            per_user,
            per_channel: false,
            last_execution: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    fn make_key(&self, event: &BotEvent) -> Option<String> {
        match event {
            BotEvent::ChatMessage { platform, channel, user, .. } => {
                let mut key = platform.clone();
                
                if self.per_channel {
                    key.push_str(":");
                    key.push_str(channel);
                }
                
                if self.per_user {
                    key.push_str(":");
                    key.push_str(user);
                }
                
                Some(key)
            }
            _ => None,
        }
    }
}

#[async_trait]
impl EventFilter for CooldownFilter {
    fn id(&self) -> &str {
        "cooldown_filter"
    }

    fn name(&self) -> &str {
        "Cooldown Filter"
    }

    fn configure(&mut self, config: serde_json::Value) -> Result<(), Error> {
        let config: CooldownFilterConfig = serde_json::from_value(config)
            .map_err(|e| Error::Platform(format!("Invalid cooldown filter config: {}", e)))?;
        
        self.cooldown_seconds = config.cooldown_seconds;
        self.per_user = config.per_user;
        self.per_channel = config.per_channel;
        Ok(())
    }

    async fn apply(&self, event: &BotEvent, _context: &EventContext) -> Result<FilterResult, Error> {
        let key = match self.make_key(event) {
            Some(k) => k,
            None => return Ok(FilterResult::Pass), // Pass non-chat events
        };
        
        let now = Utc::now();
        let cooldown_duration = Duration::seconds(self.cooldown_seconds);
        
        let mut last_execution = self.last_execution.write().await;
        
        if let Some(last_time) = last_execution.get(&key) {
            if now - *last_time < cooldown_duration {
                return Ok(FilterResult::Reject);
            }
        }
        
        // Update last execution time
        last_execution.insert(key, now);
        
        // Clean up old entries to prevent memory growth
        if last_execution.len() > 1000 {
            let cutoff = now - Duration::hours(1);
            last_execution.retain(|_, time| *time > cutoff);
        }
        
        Ok(FilterResult::Pass)
    }
}