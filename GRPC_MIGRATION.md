# gRPC Migration Documentation

## Overview

This document describes the successful migration of all TUI commands from direct BotApi usage to a gRPC-based architecture using a shared client connection.

## Architecture Changes

### Before
- TUI commands directly called BotApi trait methods
- Commands were tightly coupled to the server implementation
- No separation between business logic and presentation

### After
- All commands use gRPC services through `GrpcClient`
- Clean separation between:
  - **Common-UI**: Business logic and gRPC calls (returns structured data)
  - **TUI Adapters**: Presentation logic (formats data for console output)
- Single shared gRPC connection for all commands

## Migration Pattern

Each command was migrated following this pattern:

### 1. Common-UI Handler (`maowbot-common-ui/src/commands/<command>.rs`)
```rust
pub struct CommandResult {
    pub field1: String,
    pub field2: Vec<Item>,
    // ... structured data
}

pub struct Commands;

impl Commands {
    pub async fn operation(
        client: &GrpcClient,
        param: &str,
    ) -> Result<CommandResult, CommandError> {
        // Make gRPC call
        // Return structured data
    }
}
```

### 2. TUI Adapter (`maowbot-tui/src/commands/<command>_adapter.rs`)
```rust
pub async fn handle_command(args: &[&str], client: &GrpcClient) -> String {
    match Commands::operation(client, args[0]).await {
        Ok(result) => format_result(&result),
        Err(e) => format!("Error: {}", e),
    }
}
```

## Commands Migrated

All commands have been successfully converted:

1. **account** - Credential management operations
2. **ai** - AI service configuration and testing  
3. **config** - Bot configuration management
4. **plugin** - Plugin listing and status checking
5. **connectivity** - Platform connection management (autostart, start, stop, chat)
6. **drip** - VRChat avatar management
7. **member** - User management and analysis
8. **osc** - Open Sound Control service management
9. **vrchat** - VRChat-specific operations (world, avatar, instance)

Previously converted:
- **user** - User CRUD operations
- **platform** - Platform configuration
- **ttv** - Twitch operations
- **discord** - Discord operations
- **command** - Command management
- **redeem** - Redeem management

## Usage

### Standalone TUI Client
```bash
# Run the standalone gRPC TUI client
cargo run -p maowbot-tui --bin tui-grpc

# Or use the convenience script
./run-tui.sh

# Connect to a specific server (future feature)
cargo run -p maowbot-tui --bin tui-grpc -- --server-addr https://192.168.1.100:9999
```

### Server with Built-in TUI (Removed)
The `--tui` flag has been removed from the server. Use the standalone TUI client instead.

## Technical Details

### TLS/Certificate Handling
The gRPC server uses self-signed certificates for TLS. The standalone TUI client will:
1. First look for `certs/server.crt` in the current directory
2. If not found, connect anyway (development mode only)

The server generates certificates in `target/debug/certs/` on first run.

## Benefits

1. **Separation of Concerns**: Business logic separated from presentation
2. **Reusability**: Common-UI handlers can be used by GUI applications
3. **Network Transparency**: TUI can connect to remote servers
4. **Testability**: Easier to test business logic without UI concerns
5. **Maintainability**: Changes to gRPC services only require updates in one place

## Future Work

1. **Remove BotApi Wrapper**: ✅ Completed - The old BotApi wrapper has been removed from the standalone TUI
2. **Server Management**: Add ability for TUI to start/stop server process
3. **GUI Implementation**: Build a GUI using the same common-ui handlers
4. **Enhanced Error Handling**: Improve error messages and recovery
5. **Performance Optimization**: Add caching and batch operations where appropriate

## Breaking Changes

- The TUI is now a separate binary that requires the server to be running
- Commands that previously had direct database access now go through gRPC
- Some command outputs may have slightly different formatting

## Migration Guide for Plugin Developers

If you have custom plugins that use the TUI:

1. Create a gRPC service definition in `maowbot-proto`
2. Implement the service in `maowbot-server/src/grpc_services/`
3. Add a command handler in `maowbot-common-ui/src/commands/`
4. Add a TUI adapter in `maowbot-tui/src/commands/`
5. Update the dispatcher to route your command

See any of the migrated commands for examples of the pattern.