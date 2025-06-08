# Event Handler Migration Guide

This document explains the new event handler architecture and how to migrate existing event handlers.

## Overview

The new event handler system provides:
- **Dynamic registration** - Handlers can be added/removed at runtime
- **Unified interface** - All handlers implement the same trait
- **Priority system** - Control execution order
- **Plugin support** - External plugins can register handlers
- **Better testability** - Handlers are isolated and mockable

## Core Components

### 1. EventContext
Encapsulates all services that handlers might need:
```rust
pub struct EventContext {
    pub platform_manager: Arc<PlatformManager>,
    pub user_service: Arc<UserService>,
    pub redeem_service: Arc<RedeemService>,
    // ... other services
}
```

### 2. EventHandler Trait
```rust
#[async_trait]
pub trait EventHandler: Send + Sync {
    fn id(&self) -> &str;
    fn event_types(&self) -> Vec<String>;
    fn platforms(&self) -> Vec<Platform>;
    async fn handle(&self, event: &BotEvent, ctx: &EventContext) -> Result<bool, Error>;
    fn priority(&self) -> i32 { 100 }
    fn is_enabled(&self) -> bool { true }
}
```

### 3. EventHandlerRegistry
Manages handler registration and lookup:
```rust
let registry = EventHandlerRegistry::new();
registry.register(Arc::new(MyHandler)).await?;
```

## Migration Example

### Before (Old Style)
```rust
// In eventsub_service.rs
match twitch_evt {
    TwitchEventSubData::StreamOnline(ev) => {
        if let Err(e) = stream_online_actions::handle_stream_online(
            ev,
            &*self.redeem_service,
            &*self.platform_manager,
            &*self.user_service,
            &*self.bot_config_repo,
            &*self.discord_repo,
        ).await {
            error!("Error handling stream.online: {:?}", e);
        }
    }
}
```

### After (New Style)
```rust
// In event_handlers/twitch/stream_online.rs
pub struct StreamOnlineHandler;

#[async_trait]
impl EventHandler for StreamOnlineHandler {
    fn id(&self) -> &str {
        "twitch.stream.online"
    }

    fn event_types(&self) -> Vec<String> {
        vec!["stream.online".to_string()]
    }

    fn platforms(&self) -> Vec<Platform> {
        vec![Platform::Twitch, Platform::TwitchEventSub]
    }

    async fn handle(&self, event: &BotEvent, ctx: &EventContext) -> Result<bool, Error> {
        match event {
            BotEvent::TwitchEventSub(TwitchEventSubData::StreamOnline(evt)) => {
                // Handler logic using ctx instead of individual parameters
                self.handle_typed(evt, ctx).await?;
                Ok(true)
            }
            _ => Ok(false),
        }
    }
}
```

## Creating New Handlers

1. **Create handler struct**:
```rust
pub struct MyCustomHandler {
    config: MyConfig,
}
```

2. **Implement EventHandler**:
```rust
#[async_trait]
impl EventHandler for MyCustomHandler {
    fn id(&self) -> &str { "my.custom.handler" }
    
    fn event_types(&self) -> Vec<String> {
        vec!["chat.message".to_string()]
    }
    
    fn platforms(&self) -> Vec<Platform> {
        vec![Platform::Twitch, Platform::Discord]
    }
    
    async fn handle(&self, event: &BotEvent, ctx: &EventContext) -> Result<bool, Error> {
        // Your handler logic here
        Ok(true)
    }
}
```

3. **Register the handler**:
```rust
let handler = Arc::new(MyCustomHandler::new(config));
registry.register(handler).await?;
```

## Using EventSubServiceV2

The new EventSubServiceV2 automatically dispatches events to registered handlers:

```rust
// Create the service
let event_context = EventContext::new(/* services */);
let registry = Arc::new(EventHandlerRegistry::new());
let service = EventSubServiceV2::new(event_bus, registry, Arc::new(event_context));

// Initialize built-in handlers
service.initialize().await?;

// Register custom handlers
service.register_handler(Arc::new(MyHandler)).await?;

// Start processing events
service.start().await;
```

## Benefits

1. **Modularity** - Each handler is self-contained
2. **Testability** - Mock EventContext for unit tests
3. **Extensibility** - Add new handlers without modifying core code
4. **Plugin Support** - External plugins can register handlers via gRPC
5. **Performance** - Handlers run in priority order, can short-circuit

## Future: Event Action Pipeline

The next phase will introduce an event action pipeline:

```rust
// Conceptual example
pipeline
    .add_filter(UserRoleFilter::new(vec!["moderator"]))
    .add_action(LogAction::new())
    .add_action(DiscordNotifyAction::new())
    .add_action(OSCTriggerAction::new())
    .build();
```

This will enable:
- Pre/post processors
- Conditional execution
- Action composition
- Plugin-provided actions

## Testing

```rust
#[tokio::test]
async fn test_my_handler() {
    // Create mock context
    let mut mock_ctx = MockEventContext::new();
    mock_ctx.expect_user_service()
        .returning(|_| Ok(test_user()));
    
    // Create handler
    let handler = MyHandler::new();
    
    // Test event handling
    let event = BotEvent::ChatMessage { /* ... */ };
    let result = handler.handle(&event, &mock_ctx).await;
    assert!(result.is_ok());
}
```