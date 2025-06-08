use async_trait::async_trait;
use crate::Error;
use crate::eventbus::BotEvent;
use crate::services::event_context::EventContext;
use maowbot_common::models::platform::Platform;

/// Result of applying a filter
#[derive(Debug, Clone, PartialEq)]
pub enum FilterResult {
    /// Event passes the filter, continue processing
    Pass,
    /// Event does not pass the filter, skip this pipeline
    Reject,
}

/// Trait for pipeline filters
#[async_trait]
pub trait EventFilter: Send + Sync {
    /// Unique identifier for this filter
    fn id(&self) -> &str;
    
    /// Human-readable name for this filter
    fn name(&self) -> &str;
    
    /// Configure the filter from JSON configuration
    fn configure(&mut self, config: serde_json::Value) -> Result<(), Error> {
        // Default implementation does nothing
        Ok(())
    }
    
    /// Apply the filter to an event
    async fn apply(&self, event: &BotEvent, context: &EventContext) -> Result<FilterResult, Error>;
}

// Built-in filters

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
        "builtin.platform"
    }

    fn name(&self) -> &str {
        "Platform Filter"
    }

    async fn apply(&self, event: &BotEvent, _context: &EventContext) -> Result<FilterResult, Error> {
        let platform = match event {
            BotEvent::ChatMessage { platform, .. } => Platform::from_string(platform),
            BotEvent::TwitchEventSub(_) => Platform::TwitchEventSub,
            _ => return Ok(FilterResult::Reject),
        };

        if self.platforms.contains(&platform) {
            Ok(FilterResult::Pass)
        } else {
            Ok(FilterResult::Reject)
        }
    }
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
        "builtin.channel"
    }

    fn name(&self) -> &str {
        "Channel Filter"
    }

    async fn apply(&self, event: &BotEvent, _context: &EventContext) -> Result<FilterResult, Error> {
        match event {
            BotEvent::ChatMessage { channel, .. } => {
                if self.channels.iter().any(|c| c == channel) {
                    Ok(FilterResult::Pass)
                } else {
                    Ok(FilterResult::Reject)
                }
            }
            _ => Ok(FilterResult::Reject),
        }
    }
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
        "builtin.user_role"
    }

    fn name(&self) -> &str {
        "User Role Filter"
    }

    async fn apply(&self, event: &BotEvent, context: &EventContext) -> Result<FilterResult, Error> {
        match event {
            BotEvent::ChatMessage { platform, user, .. } => {
                // Get user from database
                let user_record = context.user_service
                    .get_or_create_user(platform, user, None)
                    .await?;
                
                // get_or_create_user returns a user, not an Option
                let _user_data = user_record;
                // Get user roles - this would need to be implemented
                // let user_roles = context.user_service.get_user_roles(user_data.user_id).await?;
                
                // For now, just pass
                Ok(FilterResult::Pass)
            }
            _ => Ok(FilterResult::Reject),
        }
    }
}

/// Filter by message content pattern
pub struct MessagePatternFilter {
    patterns: Vec<regex::Regex>,
    match_any: bool,
}

impl MessagePatternFilter {
    pub fn new(patterns: Vec<&str>, match_any: bool) -> Result<Self, Error> {
        let compiled_patterns = patterns
            .into_iter()
            .map(|p| regex::Regex::new(p))
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
        "builtin.message_pattern"
    }

    fn name(&self) -> &str {
        "Message Pattern Filter"
    }

    async fn apply(&self, event: &BotEvent, _context: &EventContext) -> Result<FilterResult, Error> {
        match event {
            BotEvent::ChatMessage { text, .. } => {
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

/// Filter by time window
pub struct TimeWindowFilter {
    start_hour: u8,
    end_hour: u8,
    timezone: chrono_tz::Tz,
}

impl TimeWindowFilter {
    pub fn new(start_hour: u8, end_hour: u8, timezone: chrono_tz::Tz) -> Self {
        Self {
            start_hour,
            end_hour,
            timezone,
        }
    }
}

#[async_trait]
impl EventFilter for TimeWindowFilter {
    fn id(&self) -> &str {
        "builtin.time_window"
    }

    fn name(&self) -> &str {
        "Time Window Filter"
    }

    async fn apply(&self, _event: &BotEvent, _context: &EventContext) -> Result<FilterResult, Error> {
        use chrono::Timelike;
        let now = chrono::Utc::now().with_timezone(&self.timezone);
        let hour = now.hour() as u8;
        
        let in_window = if self.start_hour <= self.end_hour {
            hour >= self.start_hour && hour < self.end_hour
        } else {
            // Handle wrap around midnight
            hour >= self.start_hour || hour < self.end_hour
        };
        
        if in_window {
            Ok(FilterResult::Pass)
        } else {
            Ok(FilterResult::Reject)
        }
    }
}

/// Composite filter that combines multiple filters
pub struct CompositeFilter {
    filters: Vec<Box<dyn EventFilter>>,
    require_all: bool, // true = AND, false = OR
}

impl CompositeFilter {
    pub fn new(require_all: bool) -> Self {
        Self {
            filters: Vec::new(),
            require_all,
        }
    }
    
    pub fn add_filter(mut self, filter: Box<dyn EventFilter>) -> Self {
        self.filters.push(filter);
        self
    }
}

#[async_trait]
impl EventFilter for CompositeFilter {
    fn id(&self) -> &str {
        "builtin.composite"
    }

    fn name(&self) -> &str {
        "Composite Filter"
    }

    async fn apply(&self, event: &BotEvent, context: &EventContext) -> Result<FilterResult, Error> {
        if self.filters.is_empty() {
            return Ok(FilterResult::Pass);
        }
        
        let mut pass_count = 0;
        
        for filter in &self.filters {
            match filter.apply(event, context).await? {
                FilterResult::Pass => {
                    pass_count += 1;
                    if !self.require_all {
                        // OR mode - one pass is enough
                        return Ok(FilterResult::Pass);
                    }
                }
                FilterResult::Reject => {
                    if self.require_all {
                        // AND mode - one reject fails all
                        return Ok(FilterResult::Reject);
                    }
                }
            }
        }
        
        // For AND mode, we need all to pass
        if self.require_all && pass_count == self.filters.len() {
            Ok(FilterResult::Pass)
        } else if !self.require_all && pass_count > 0 {
            Ok(FilterResult::Pass)
        } else {
            Ok(FilterResult::Reject)
        }
    }
}