# AI Configuration Fix and API Key Management

## Problem Summary

The AI service was showing as configured but returning "AI service not configured" errors when accessed through gRPC. The issue was that the gRPC service layer (`AiServiceImpl`) wasn't receiving the configured AI service implementation from the server context.

## Root Cause

In `maowbot-server/src/server.rs`, the AI gRPC service was created with the default constructor:
```rust
.add_service(AiServiceServer::new(AiServiceImpl::new()))
```

This created an `AiServiceImpl` with `ai_api: None`, disconnecting it from the actual AI service configured in the server context.

## Fix Applied

Modified the server initialization to properly pass the AI API implementation:
```rust
.add_service(AiServiceServer::new({
    if let Some(ref ai_api_impl) = ctx.plugin_manager.ai_api_impl {
        AiServiceImpl::new_with_api(Arc::new(ai_api_impl.clone()))
    } else {
        AiServiceImpl::new()
    }
}))
```

## API Key Management System

### Current Implementation

The system already supports multiple sources for API keys with proper encryption:

1. **Database Storage** (Primary)
   - Table: `ai_credentials` 
   - Encryption: AES-GCM encryption via `Encryptor` class
   - Keys are encrypted before storage and decrypted on retrieval

2. **Environment Variables** (Fallback)
   - Checked during server startup if no database configuration exists
   - Currently supports: `OPENAI_API_KEY`
   - Automatically saves to database when found

3. **TUI Configuration** (Runtime)
   - Command: `ai provider configure <provider> --api-key <KEY>`
   - Stores encrypted keys in database
   - Persists across server restarts

### Configuration Flow

```
Server Startup
    ↓
Load from Database (ai_credentials table)
    ↓ (if empty)
Check Environment Variables
    ↓ (if found)
Save to Database (encrypted)
    ↓
AI Service Ready
```

### Available TUI Commands

```bash
# Check AI status
ai status

# List available providers
ai provider list

# Configure a provider
ai provider configure openai --api-key YOUR_KEY --model gpt-4o
ai provider configure anthropic --api-key YOUR_KEY --model claude-3-opus

# Test a provider
ai provider test openai "hello"

# Use AI chat
ai chat "Your message here"

# Manage prompts
ai prompt list
ai prompt set <name> <prompt>
ai prompt get <name>
```

### Database Schema

The `ai_credentials` table (from migration 006_ai_init.sql):
```sql
CREATE TABLE ai_credentials (
    ai_credential_id UUID PRIMARY KEY,
    provider VARCHAR(50) NOT NULL,
    api_key_encrypted BYTEA NOT NULL,
    api_key_nonce BYTEA NOT NULL,
    default_model VARCHAR(100),
    api_base_url VARCHAR(255),
    additional_config JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(provider)
);
```

### Security Features

1. **Encryption**: All API keys are encrypted using AES-GCM
2. **Nonce**: Each encrypted key has a unique nonce for security
3. **No Plain Text**: Keys are never stored in plain text
4. **Memory Safety**: Keys are only decrypted when needed by the AI service

### Future Subscription Service Support

The system is already designed to support subscription services:

1. **External API Keys**: Can be loaded from external sources via the database
2. **Multiple Providers**: Supports configuring multiple AI providers
3. **Dynamic Configuration**: Providers can be configured at runtime
4. **Per-User Keys**: The architecture supports per-user API key management (future enhancement)

### Testing

Use the provided test script to verify functionality:
```bash
./test_ai_config.sh
```

Note: Replace test keys with real API keys for actual testing.

## Implementation Details

### File Structure

- **AI Service**: `maowbot-ai/src/plugins/ai_service.rs`
- **gRPC Service**: `maowbot-server/src/grpc_services/ai_service.rs`
- **Database Repository**: `maowbot-core/src/repositories/postgres/ai.rs`
- **Models**: `maowbot-common/src/models/ai.rs`
- **Server Context**: `maowbot-server/src/context.rs`

### Key Methods

1. **configure_provider()**: Stores encrypted API keys in database
2. **initialize_from_database()**: Loads saved configurations on startup
3. **get_or_create_encryptor()**: Manages encryption/decryption
4. **test_provider()**: Validates API key by making a test request