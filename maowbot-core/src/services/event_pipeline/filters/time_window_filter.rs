use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use chrono_tz::Tz;
use crate::Error;
use crate::eventbus::BotEvent;
use crate::services::event_context::EventContext;
use crate::services::event_pipeline::{EventFilter, FilterResult};

#[derive(Debug, Serialize, Deserialize)]
struct TimeWindowFilterConfig {
    #[serde(default)]
    start_hour: u8,
    #[serde(default = "default_end_hour")]
    end_hour: u8,
    #[serde(default = "default_timezone")]
    timezone: String,
    #[serde(default)]
    days_of_week: Vec<u8>, // 0 = Monday, 6 = Sunday
}

fn default_end_hour() -> u8 {
    23
}

fn default_timezone() -> String {
    "UTC".to_string()
}

/// Filter by time window
pub struct TimeWindowFilter {
    start_hour: u8,
    end_hour: u8,
    timezone: Tz,
    days_of_week: Vec<u8>,
}

impl TimeWindowFilter {
    pub fn new(start_hour: u8, end_hour: u8, timezone: String) -> Self {
        let tz = timezone.parse::<Tz>().unwrap_or(Tz::UTC);
        Self {
            start_hour,
            end_hour,
            timezone: tz,
            days_of_week: vec![],
        }
    }
}

#[async_trait]
impl EventFilter for TimeWindowFilter {
    fn id(&self) -> &str {
        "time_window_filter"
    }

    fn name(&self) -> &str {
        "Time Window Filter"
    }

    fn configure(&mut self, config: serde_json::Value) -> Result<(), Error> {
        let config: TimeWindowFilterConfig = serde_json::from_value(config)
            .map_err(|e| Error::Platform(format!("Invalid time window filter config: {}", e)))?;
        
        self.start_hour = config.start_hour;
        self.end_hour = config.end_hour;
        self.timezone = config.timezone.parse::<Tz>()
            .map_err(|_| Error::Platform(format!("Invalid timezone: {}", config.timezone)))?;
        self.days_of_week = config.days_of_week;
        Ok(())
    }

    async fn apply(&self, _event: &BotEvent, _context: &EventContext) -> Result<FilterResult, Error> {
        use chrono::{Timelike, Datelike};
        let now = chrono::Utc::now().with_timezone(&self.timezone);
        let hour = now.hour() as u8;
        let weekday = now.weekday().num_days_from_monday() as u8;
        
        // Check day of week if specified
        if !self.days_of_week.is_empty() && !self.days_of_week.contains(&weekday) {
            return Ok(FilterResult::Reject);
        }
        
        // Check hour window
        let in_window = if self.start_hour <= self.end_hour {
            hour >= self.start_hour && hour <= self.end_hour
        } else {
            // Handle wrap around midnight
            hour >= self.start_hour || hour <= self.end_hour
        };
        
        if in_window {
            Ok(FilterResult::Pass)
        } else {
            Ok(FilterResult::Reject)
        }
    }
}