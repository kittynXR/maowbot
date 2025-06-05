# AI Service Enable/Disable and Provider Keys Update

## Summary

Successfully implemented the following AI service enhancements:

### 1. **Enable/Disable Functionality**
- Added `ai enable` and `ai disable` commands to control the AI service
- These commands work through the gRPC interface 
- The AI service maintains an enabled/disabled state that persists

### 2. **Show Provider Keys**
- Added `ai provider show [provider-name]` command to display configured API keys
- API keys are shown with security masking (only last 4 characters visible)
- Shows additional info: API base URL, active status, configuration timestamp

### 3. **Fixed gRPC Connection Issue**
- Fixed the server initialization to properly pass the AI service to the gRPC layer
- The issue was in `maowbot-server/src/server.rs` where the AI service wasn't being connected

## Implementation Details

### Proto Changes (`ai_service.proto`)
```protobuf
// Service control
rpc EnableAI(EnableAIRequest) returns (EnableAIResponse);
rpc DisableAI(DisableAIRequest) returns (DisableAIResponse);
rpc GetAIStatus(GetAIStatusRequest) returns (GetAIStatusResponse);
rpc ShowProviderKeys(ShowProviderKeysRequest) returns (ShowProviderKeysResponse);
```

### Server Fix (`server.rs`)
```rust
// Before - AI service was not connected
.add_service(AiServiceServer::new(AiServiceImpl::new()))

// After - AI service properly connected
.add_service(AiServiceServer::new({
    if let Some(ref ai_api_impl) = ctx.plugin_manager.ai_api_impl {
        AiServiceImpl::new_with_api(Arc::new(ai_api_impl.clone()))
    } else {
        AiServiceImpl::new()
    }
}))
```

### Available Commands

```bash
# Enable/disable AI service
ai enable
ai disable

# Check AI service status
ai status

# Show provider API keys (masked for security)
ai provider show              # Show all configured providers
ai provider show openai       # Show only OpenAI configuration

# Configure providers (existing functionality)
ai provider configure openai --api-key sk-... --model gpt-4
ai provider configure anthropic --api-key sk-ant-...

# Test providers
ai provider test openai "hello"

# Chat with AI
ai chat "Hello, how are you?"
```

## Security Features

1. **API Key Masking**: When displaying API keys, only the last 4 characters are shown (e.g., `...abcd`)
2. **Encrypted Storage**: API keys continue to be stored encrypted in the database
3. **No Plain Text Display**: Full API keys are never displayed in the TUI

## Architecture Notes

- The AI service maintains its state internally
- Enable/disable commands affect all AI processing globally
- Provider configurations persist in the database with encryption
- The gRPC layer now properly communicates with the AI service implementation

## Testing

To test the new functionality:

```bash
# Start the server
cd maowbot-server && cargo run

# In another terminal, run the TUI
./run-tui.sh

# Test commands
tui> ai status
tui> ai enable
tui> ai provider show
tui> ai provider configure openai --api-key YOUR_KEY
tui> ai chat "Hello!"
tui> ai disable
```

## Future Enhancements

- Add per-user API key management for subscription services
- Add usage tracking and limits
- Add provider health monitoring
- Add automatic failover between providers