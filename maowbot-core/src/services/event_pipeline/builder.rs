use crate::services::event_pipeline::{
    EventPipeline, EventFilter, EventAction,
    filter::{
        PlatformFilter, ChannelFilter, UserRoleFilter, MessagePatternFilter,
        TimeWindowFilter, CompositeFilter,
    },
    action::{
        LogAction, DiscordMessageAction, OSCTriggerAction, PluginAction,
    },
};
use maowbot_common::models::platform::Platform;

/// Builder for creating event pipelines with a fluent API
pub struct PipelineBuilder {
    pipeline: EventPipeline,
}

impl PipelineBuilder {
    /// Create a new pipeline builder
    pub fn new(id: &str, name: &str) -> Self {
        Self {
            pipeline: EventPipeline::new(id, name),
        }
    }

    /// Set pipeline priority (lower numbers execute first)
    pub fn priority(mut self, priority: i32) -> Self {
        self.pipeline.priority = priority;
        self
    }

    /// Set whether pipeline is enabled
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.pipeline.enabled = enabled;
        self
    }

    /// Set whether to stop processing other pipelines after this one
    pub fn stop_on_match(mut self, stop: bool) -> Self {
        self.pipeline.stop_on_match = stop;
        self
    }

    /// Add a custom filter
    pub fn filter(mut self, filter: Box<dyn EventFilter>) -> Self {
        self.pipeline.filters.push(filter);
        self
    }

    /// Add a platform filter
    pub fn platform(self, platforms: Vec<Platform>) -> Self {
        self.filter(Box::new(PlatformFilter::new(platforms)))
    }

    /// Add a channel filter
    pub fn channel(self, channels: Vec<&str>) -> Self {
        let channels: Vec<String> = channels.into_iter().map(|s| s.to_string()).collect();
        self.filter(Box::new(ChannelFilter::new(channels)))
    }

    /// Add a user role filter
    pub fn user_roles(self, roles: Vec<&str>, match_any: bool) -> Self {
        let roles: Vec<String> = roles.into_iter().map(|s| s.to_string()).collect();
        self.filter(Box::new(UserRoleFilter::new(roles, match_any)))
    }

    /// Add a message pattern filter
    pub fn message_pattern(self, patterns: Vec<&str>, match_any: bool) -> Result<Self, crate::Error> {
        let filter = MessagePatternFilter::new(patterns, match_any)?;
        Ok(self.filter(Box::new(filter)))
    }

    /// Add a time window filter
    pub fn time_window(self, start_hour: u8, end_hour: u8, timezone: chrono_tz::Tz) -> Self {
        self.filter(Box::new(TimeWindowFilter::new(start_hour, end_hour, timezone)))
    }

    /// Add a composite filter
    pub fn composite_filter<F>(self, require_all: bool, builder: F) -> Self
    where
        F: FnOnce(CompositeFilterBuilder) -> CompositeFilterBuilder,
    {
        let composite_builder = CompositeFilterBuilder::new(require_all);
        let built = builder(composite_builder);
        self.filter(Box::new(built.build()))
    }

    /// Add a custom action
    pub fn action(mut self, action: Box<dyn EventAction>) -> Self {
        self.pipeline.actions.push(action);
        self
    }

    /// Add a log action
    pub fn log(self, level: tracing::Level) -> Self {
        self.action(Box::new(LogAction::new(level)))
    }

    /// Add a Discord message action
    pub fn discord_message(self, account: &str, channel_id: &str, template: &str) -> Self {
        self.action(Box::new(DiscordMessageAction::new(account, channel_id, template)))
    }

    /// Add an OSC trigger action
    pub fn osc_trigger(self, parameter: &str, value: f32, duration_ms: Option<u64>) -> Self {
        self.action(Box::new(OSCTriggerAction::new(parameter, value, duration_ms)))
    }

    /// Add a plugin action
    pub fn plugin_action<F>(self, plugin_id: &str, function: &str, params: F) -> Self
    where
        F: FnOnce(PluginActionBuilder) -> PluginActionBuilder,
    {
        let action_builder = PluginActionBuilder::new(plugin_id, function);
        let built = params(action_builder);
        self.action(Box::new(built.build()))
    }

    /// Build the pipeline
    pub fn build(self) -> EventPipeline {
        self.pipeline
    }
}

/// Builder for composite filters
pub struct CompositeFilterBuilder {
    filter: CompositeFilter,
}

impl CompositeFilterBuilder {
    fn new(require_all: bool) -> Self {
        Self {
            filter: CompositeFilter::new(require_all),
        }
    }

    pub fn add(self, filter: Box<dyn EventFilter>) -> Self {
        Self {
            filter: self.filter.add_filter(filter),
        }
    }

    pub fn platform(self, platforms: Vec<Platform>) -> Self {
        self.add(Box::new(PlatformFilter::new(platforms)))
    }

    pub fn channel(self, channels: Vec<&str>) -> Self {
        let channels: Vec<String> = channels.into_iter().map(|s| s.to_string()).collect();
        self.add(Box::new(ChannelFilter::new(channels)))
    }

    fn build(self) -> CompositeFilter {
        self.filter
    }
}

/// Builder for plugin actions
pub struct PluginActionBuilder {
    action: PluginAction,
}

impl PluginActionBuilder {
    fn new(plugin_id: &str, function: &str) -> Self {
        Self {
            action: PluginAction::new(plugin_id, function),
        }
    }

    pub fn param(self, key: &str, value: &str) -> Self {
        Self {
            action: self.action.with_parameter(key, value),
        }
    }

    fn build(self) -> PluginAction {
        self.action
    }
}

// Example usage:
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pipeline_builder() {
        let pipeline = PipelineBuilder::new("test_pipeline", "Test Pipeline")
            .priority(50)
            .stop_on_match(true)
            .platform(vec![Platform::Twitch, Platform::TwitchIRC])
            .channel(vec!["general", "bot-commands"])
            .message_pattern(vec![r"^!test"], false).unwrap()
            .log(tracing::Level::INFO)
            .discord_message("bot", "12345", "Test triggered by {user}: {message}")
            .osc_trigger("/avatar/parameters/happy", 1.0, Some(5000))
            .plugin_action("custom_plugin", "process_test", |p| {
                p.param("action", "test")
                 .param("level", "5")
            })
            .build();

        assert_eq!(pipeline.id, "test_pipeline");
        assert_eq!(pipeline.priority, 50);
        assert!(pipeline.stop_on_match);
        assert_eq!(pipeline.filters.len(), 3);
        assert_eq!(pipeline.actions.len(), 4);
    }

    #[test]
    fn test_composite_filter_builder() {
        let pipeline = PipelineBuilder::new("complex", "Complex Pipeline")
            .composite_filter(false, |f| { // OR mode
                f.platform(vec![Platform::Twitch])
                 .channel(vec!["special"])
            })
            .build();

        assert_eq!(pipeline.filters.len(), 1);
    }
}