# MaowBot Pipeline System Documentation

## Overview

The MaowBot Pipeline System is a flexible, event-driven architecture that processes platform events through configurable workflows. It replaces hard-coded event handlers with a dynamic system that can be modified at runtime without code changes.

## Architecture

### Core Components

1. **Event Type Registry** - Defines all possible events in the system
2. **Event Handler Registry** - Contains all available filters and actions
3. **Event Pipelines** - Workflows that process events
4. **Pipeline Filters** - Conditions that determine if a pipeline should execute
5. **Pipeline Actions** - Operations performed when a pipeline executes

### Database Schema

#### Event Type Registry
```sql
event_type_registry
├── event_type_id (UUID)
├── platform (twitch, discord, vrchat, obs, system)
├── event_category (chat, stream, user, subscription, etc)
├── event_name (message.create, stream.online, etc)
├── description
├── event_schema (JSONB) - JSON schema for validation
└── is_enabled
```

#### Event Handler Registry
```sql
event_handler_registry
├── handler_id (UUID)
├── handler_type (filter or action)
├── handler_name (unique identifier)
├── handler_category
├── description
├── parameters (JSONB) - Parameter definitions
├── is_builtin
├── plugin_id - For plugin-provided handlers
└── is_enabled
```

#### Event Pipelines
```sql
event_pipelines
├── pipeline_id (UUID)
├── name (unique)
├── description
├── enabled
├── priority (lower = higher priority)
├── stop_on_match - Stop processing other pipelines
├── stop_on_error - Stop on first error
├── is_system - Protected from deletion
├── tags (TEXT[])
├── metadata (JSONB)
└── execution statistics
```

#### Pipeline Filters
```sql
pipeline_filters
├── filter_id (UUID)
├── pipeline_id → event_pipelines
├── filter_type → event_handler_registry.handler_name
├── filter_config (JSONB) - Filter-specific configuration
├── filter_order - Execution order
├── is_negated - Invert filter result
└── is_required - Must pass for pipeline to execute
```

#### Pipeline Actions
```sql
pipeline_actions
├── action_id (UUID)
├── pipeline_id → event_pipelines
├── action_type → event_handler_registry.handler_name
├── action_config (JSONB) - Action-specific configuration
├── action_order - Execution order
├── continue_on_error
├── is_async - Execute asynchronously
├── timeout_ms
├── retry_count
├── retry_delay_ms
├── condition_type - Conditional execution
└── condition_config (JSONB)
```

## Built-in Filters

### Platform Filter
Filters events by platform.
```json
{
  "platforms": ["twitch", "discord"]
}
```

### Channel Filter
Filters events by channel name.
```json
{
  "channels": ["general", "stream-chat"]
}
```

### User Role Filter
Filters by user roles.
```json
{
  "roles": ["moderator", "vip"],
  "match_any": true
}
```

### Message Pattern Filter
Filters messages by regex patterns.
```json
{
  "patterns": ["^!", "(?i)hello"],
  "match_any": false
}
```

### Time Window Filter
Filters by time of day.
```json
{
  "start_hour": 18,
  "end_hour": 23,
  "timezone": "America/New_York"
}
```

### Cooldown Filter
Rate limiting filter.
```json
{
  "cooldown_seconds": 30,
  "per_user": true
}
```

## Built-in Actions

### Discord Actions

#### discord_message
Send a message to Discord.
```json
{
  "channel_id": "123456789",
  "message_template": "New follower: {user}!",
  "embed": {
    "title": "Stream Alert",
    "color": 0xFF0000
  }
}
```

#### discord_role_add / discord_role_remove
Manage Discord roles.
```json
{
  "role_id": "987654321",
  "user_id": "{event.user_id}"
}
```

### Twitch Actions

#### twitch_message
Send a Twitch chat message.
```json
{
  "message": "Thanks for the follow, @{user}!",
  "reply_to": "{event.message_id}"
}
```

#### twitch_timeout
Timeout a user.
```json
{
  "user_id": "{event.user_id}",
  "duration": 600,
  "reason": "Spam"
}
```

### OSC Actions

#### osc_trigger
Trigger VRChat avatar parameters.
```json
{
  "parameter_path": "/avatar/parameters/happy",
  "value": 1.0,
  "duration_ms": 5000
}
```

### OBS Actions

#### obs_scene_change
Change OBS scene.
```json
{
  "scene_name": "BRB",
  "instance": "default"
}
```

#### obs_source_toggle
Toggle source visibility.
```json
{
  "source_name": "Webcam",
  "visible": false,
  "scene": "Main"
}
```

## AI Integration

The pipeline system includes first-class AI support with actions for:

### AI Response Actions

#### ai_text_response
Generate text responses with personality.
```json
{
  "personality_id": "maow",
  "include_memory": true,
  "emotion_influence": 0.7
}
```

#### ai_voice_response
Generate voice responses with TTS.
```json
{
  "personality_id": "maow",
  "voice_emotion": "happy",
  "speed": 1.2
}
```

#### ai_analyze_input
Analyze user input for intent and emotion.
```json
{
  "analyze_emotion": true,
  "analyze_intent": true,
  "update_mood": true
}
```

### Memory Actions

#### ai_remember
Store information in AI memory.
```json
{
  "memory_type": "user_preference",
  "importance": 0.8,
  "auto_expire": false
}
```

#### ai_recall
Retrieve relevant memories.
```json
{
  "context": "{event.message}",
  "limit": 5,
  "min_importance": 0.5
}
```

## VRChat Robotics Integration

The system supports VRChat avatar control for AI personalities:

### Avatar Control Actions

#### avatar_set_emotion
Control avatar expressions.
```json
{
  "emotion": "happy",
  "intensity": 0.8,
  "duration_ms": 5000
}
```

#### avatar_perform_gesture
Trigger animations.
```json
{
  "gesture": "wave",
  "loop": false,
  "blend_time": 500
}
```

#### avatar_move_to
Move avatar in world.
```json
{
  "position": [10.5, 0, -5.2],
  "look_at": [15.0, 1.5, -5.2],
  "speed": 1.5
}
```

## Example Pipelines

### Stream Online Announcement
```sql
-- Pipeline that announces when stream goes live
Pipeline: stream_online_announcement
├── Filters:
│   └── platform_filter: {"platforms": ["twitch-eventsub"]}
└── Actions:
    ├── discord_message: Send announcement to Discord
    └── osc_trigger: Set streaming parameter to 1.0
```

### Auto-Moderation
```sql
-- Pipeline for automatic chat moderation
Pipeline: auto_moderation
├── Filters:
│   ├── platform_filter: {"platforms": ["twitch-irc"]}
│   └── message_pattern_filter: Check for spam patterns
└── Actions:
    ├── twitch_timeout: Timeout the user
    └── log_action: Log the moderation action
```

### AI Chat Response
```sql
-- Pipeline for AI-powered chat responses
Pipeline: ai_chat_response
├── Filters:
│   ├── platform_filter: {"platforms": ["twitch-irc", "discord"]}
│   └── message_pattern_filter: {"patterns": ["@maow", "hey maow"]}
└── Actions:
    ├── ai_analyze_input: Analyze message intent/emotion
    ├── ai_recall: Get relevant memories
    ├── ai_text_response: Generate response
    └── twitch_message/discord_message: Send response
```

## Creating Custom Pipelines

### Via SQL
```sql
-- Create pipeline
INSERT INTO event_pipelines (name, description, enabled, priority) 
VALUES ('my_custom_pipeline', 'Description', true, 100);

-- Add filters
INSERT INTO pipeline_filters (pipeline_id, filter_type, filter_config, filter_order)
VALUES (
  (SELECT pipeline_id FROM event_pipelines WHERE name = 'my_custom_pipeline'),
  'platform_filter',
  '{"platforms": ["twitch"]}',
  0
);

-- Add actions
INSERT INTO pipeline_actions (pipeline_id, action_type, action_config, action_order)
VALUES (
  (SELECT pipeline_id FROM event_pipelines WHERE name = 'my_custom_pipeline'),
  'discord_message',
  '{"channel_id": "123", "message_template": "Event: {event_type}"}',
  0
);
```

### Via API (Future)
```rust
// Example API usage
let pipeline = Pipeline::builder()
    .name("my_custom_pipeline")
    .add_filter(PlatformFilter::new(vec!["twitch"]))
    .add_action(DiscordMessage::new("123", "Event: {event_type}"))
    .build();
```

## Pipeline Execution Flow

1. **Event Received**: Platform event enters the system
2. **Pipeline Selection**: Active pipelines sorted by priority
3. **Filter Evaluation**: All filters must pass (unless marked optional)
4. **Action Execution**: Actions run in order
5. **Error Handling**: Continue or stop based on configuration
6. **Statistics Update**: Execution count and success rate tracked

## Performance Considerations

- Pipeline execution logs are partitioned by month
- Indexes on frequently queried fields
- Asynchronous action support for long-running operations
- Configurable timeouts and retry logic
- Pipeline statistics for monitoring

## Migration from Legacy System

The pipeline system replaces:
- Hard-coded Discord event configs
- Fixed command handlers
- Static redeem processors

Legacy systems remain for backward compatibility but new features should use pipelines.

## Best Practices

1. **Use descriptive names** for pipelines and include purpose in description
2. **Set appropriate priorities** - Lower numbers execute first
3. **Use stop_on_match** for exclusive handlers
4. **Configure timeouts** for external API calls
5. **Enable logging** for debugging
6. **Test filters thoroughly** before enabling in production
7. **Monitor execution statistics** to identify performance issues

## Future Enhancements

- Web UI for pipeline management
- Pipeline templates
- A/B testing support
- Advanced analytics
- Pipeline versioning
- Import/export functionality