# Event Action Pipeline Design

This document describes the event action pipeline system that builds on top of the event handler architecture.

## Overview

The event action pipeline provides a flexible, data-driven approach to event processing. Instead of hardcoding event handling logic, pipelines allow you to:

- Define reusable filters and actions
- Compose complex workflows from simple building blocks
- Configure event handling through data rather than code
- Enable plugin-provided actions and filters

## Architecture

### Core Components

1. **EventFilter** - Determines if a pipeline should process an event
2. **EventAction** - Performs operations when a pipeline executes
3. **EventPipeline** - Combines filters and actions into a workflow
4. **PipelineExecutor** - Manages and executes multiple pipelines
5. **PipelineBuilder** - Fluent API for constructing pipelines

### Event Flow

```
Event → PipelineExecutor → Pipeline 1 → Filters → Actions
                        ↓
                        → Pipeline 2 → Filters → Actions
                        ↓
                        → Pipeline N → Filters → Actions
```

## Filters

Filters determine whether a pipeline should process an event. All filters must pass for the pipeline to execute.

### Built-in Filters

- **PlatformFilter** - Match events from specific platforms
- **ChannelFilter** - Match events from specific channels
- **UserRoleFilter** - Match events from users with specific roles
- **MessagePatternFilter** - Match messages by regex patterns
- **TimeWindowFilter** - Match events within time ranges
- **CompositeFilter** - Combine multiple filters with AND/OR logic

### Custom Filters

```rust
#[async_trait]
impl EventFilter for MyFilter {
    fn id(&self) -> &str { "my_filter" }
    fn name(&self) -> &str { "My Custom Filter" }
    
    async fn apply(&self, event: &BotEvent, context: &EventContext) -> Result<FilterResult, Error> {
        // Your filter logic
        Ok(FilterResult::Pass)
    }
}
```

## Actions

Actions perform operations when a pipeline executes. Actions run sequentially and can share data.

### Built-in Actions

- **LogAction** - Log events at various levels
- **DiscordMessageAction** - Send Discord messages
- **OSCTriggerAction** - Trigger OSC parameters
- **PluginAction** - Execute plugin functions

### Action Results

- `Continue` - Action succeeded, continue pipeline
- `Stop` - Action succeeded, stop pipeline
- `Skip` - Action was skipped
- `Failed(String)` - Action failed with reason

### Custom Actions

```rust
#[async_trait]
impl EventAction for MyAction {
    fn id(&self) -> &str { "my_action" }
    fn name(&self) -> &str { "My Custom Action" }
    
    async fn execute(&self, context: &mut ActionContext) -> Result<ActionResult, Error> {
        // Your action logic
        Ok(ActionResult::Continue)
    }
}
```

## Pipeline Builder

The `PipelineBuilder` provides a fluent API for creating pipelines:

```rust
let pipeline = PipelineBuilder::new("example", "Example Pipeline")
    .priority(50)
    .platform(vec![Platform::Twitch])
    .channel(vec!["general"])
    .message_pattern(vec![r"^!hello"], false)?
    .log(tracing::Level::INFO)
    .discord_message("bot", "123456", "Hello from {user}!")
    .osc_trigger("/avatar/happy", 1.0, Some(5000))
    .build();
```

## Examples

### Stream Announcement Pipeline

```rust
PipelineBuilder::new("stream_announce", "Stream Announcement")
    .priority(10)
    .platform(vec![Platform::TwitchEventSub])
    .discord_message("bot", "123456789", 
        "🔴 **{broadcaster}** is now live!\n{title}")
    .osc_trigger("/avatar/streaming", 1.0, None)
    .build()
```

### Auto-Moderation Pipeline

```rust
PipelineBuilder::new("auto_mod", "Auto Moderation")
    .priority(1)
    .stop_on_match(true)
    .platform(vec![Platform::TwitchIRC])
    .message_pattern(vec![r"(?i)buy.+followers"], true)?
    .plugin_action("moderation", "timeout_user", |p| {
        p.param("duration", "600")
         .param("reason", "Suspicious link")
    })
    .build()
```

### Cross-Platform Mirror

```rust
PipelineBuilder::new("discord_mirror", "Discord to Twitch")
    .platform(vec![Platform::Discord])
    .channel(vec!["stream-chat"])
    .plugin_action("cross_platform", "mirror_message", |p| {
        p.param("target", "twitch")
         .param("format", "[Discord] {user}: {message}")
    })
    .build()
```

## Data Sharing

Actions can share data through the `ActionContext`:

```rust
// In one action:
context.set_data("user_level", 5);

// In a later action:
if let Some(level) = context.get_data::<i32>("user_level") {
    // Use the level
}
```

## Integration with Plugins

Plugins can:
1. Register custom filters and actions
2. Execute via PluginAction
3. Access shared context data
4. Integrate with other bot services

## Configuration

Pipelines can be:
- Defined in code (as shown)
- Loaded from configuration files
- Created dynamically via API
- Modified at runtime

## Performance Considerations

- Pipelines execute in priority order
- Filters short-circuit on first rejection
- Actions can be marked as parallelizable
- Use `stop_on_match` to prevent unnecessary processing

## Future Enhancements

1. **Visual Pipeline Editor** - GUI for creating pipelines
2. **Pipeline Templates** - Reusable pipeline patterns
3. **Conditional Actions** - Actions that run based on previous results
4. **Pipeline Metrics** - Track execution times and success rates
5. **Hot Reload** - Update pipelines without restart
6. **Pipeline Testing** - Test pipelines with simulated events

## Migration Path

1. Start with simple handler registration (current system)
2. Gradually move complex handlers to pipelines
3. Enable user-defined pipelines
4. Full pipeline-based configuration

The pipeline system complements the event handler system, providing more flexibility for complex workflows while maintaining the simplicity of basic handlers.