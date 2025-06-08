use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use crate::Error;
use crate::services::event_pipeline::{EventAction, ActionResult, ActionContext};

#[derive(Debug, Serialize, Deserialize)]
struct LogActionConfig {
    #[serde(default = "default_level")]
    level: String,
    #[serde(default)]
    prefix: String,
}

fn default_level() -> String {
    "info".to_string()
}

/// Action that logs event details
pub struct LogAction {
    level: String,
    prefix: String,
}

impl LogAction {
    pub fn new(level: String) -> Self {
        Self {
            level,
            prefix: String::new(),
        }
    }
    
    fn get_log_level(&self) -> tracing::Level {
        match self.level.to_lowercase().as_str() {
            "error" => tracing::Level::ERROR,
            "warn" => tracing::Level::WARN,
            "info" => tracing::Level::INFO,
            "debug" => tracing::Level::DEBUG,
            "trace" => tracing::Level::TRACE,
            _ => tracing::Level::INFO,
        }
    }
}

#[async_trait]
impl EventAction for LogAction {
    fn id(&self) -> &str {
        "log_action"
    }

    fn name(&self) -> &str {
        "Log Event"
    }

    fn configure(&mut self, config: serde_json::Value) -> Result<(), Error> {
        let config: LogActionConfig = serde_json::from_value(config)
            .map_err(|e| Error::Platform(format!("Invalid log action config: {}", e)))?;
        
        self.level = config.level;
        self.prefix = config.prefix;
        Ok(())
    }

    async fn execute(&self, context: &mut ActionContext) -> Result<ActionResult, Error> {
        let level = self.get_log_level();
        let prefix = if self.prefix.is_empty() {
            "Event Pipeline".to_string()
        } else {
            self.prefix.clone()
        };
        
        match level {
            tracing::Level::ERROR => tracing::error!("{}: {:?}", prefix, context.event),
            tracing::Level::WARN => tracing::warn!("{}: {:?}", prefix, context.event),
            tracing::Level::INFO => tracing::info!("{}: {:?}", prefix, context.event),
            tracing::Level::DEBUG => tracing::debug!("{}: {:?}", prefix, context.event),
            tracing::Level::TRACE => tracing::trace!("{}: {:?}", prefix, context.event),
        }
        
        Ok(ActionResult::Success(serde_json::json!({
            "logged": true,
            "level": self.level,
            "event_type": context.event.event_type()
        })))
    }

    fn is_parallelizable(&self) -> bool {
        true
    }
}