# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

MaowBot is a multi-platform bot and plugin framework in preproduction. It integrates with chat platforms (Twitch, Discord, VRChat) and supports both in-process and gRPC-based plugins. The project is a Rust workspace with PostgreSQL for persistence.

## Build Commands

```bash
# Build entire workspace
cargo build --all

# Build specific crate
cargo build -p maowbot-core

# Run tests for entire workspace
cargo test --all

# Run tests for specific crate
cargo test -p maowbot-core

# Run a single test
cargo test test_name

# Run integration tests only
cargo test --test '*'

# Run the main server
cd maowbot-server && cargo run

# Run with verbose logging
RUST_LOG=debug cargo run

# Run the standalone TUI client (requires server to be running)
cargo run -p maowbot-tui --bin maowbot-tui-grpc
# Or use the convenience script:
./run-tui.sh
```

## Architecture

### Core Components

1. **maowbot-core**: Main library containing:
   - `platforms/`: Platform integrations (Twitch IRC, EventSub, Discord via Twilight, VRChat)
   - `auth/`: OAuth flows and credential management with encryption
   - `services/`: Message handling, command/redeem processing
   - `repositories/`: PostgreSQL data layer using SQLx
   - `plugins/`: Plugin management supporting both in-process and gRPC plugins
   - `tasks/`: Background tasks (credential refresh, maintenance)

2. **maowbot-proto**: gRPC definitions for services and remote plugins
   - Service definitions for all bot operations (user, platform, twitch, discord, etc.)
   - Plugin communication via bidirectional streaming

3. **maowbot-server**: Core server application
   - Hosts gRPC services for all bot operations
   - Runs database migrations on startup
   - Manages platform connections and plugin loading
   - Provides gRPC API for all bot operations

4. **maowbot-common-ui**: Shared UI business logic
   - gRPC client wrapper with connection pooling
   - Command handlers that return structured data
   - Unified tab completion system with context awareness
   - Process manager for server and overlay control
   - Used by both TUI and future GUI applications

5. **maowbot-tui**: Terminal User Interface
   - Standalone gRPC client binary: `maowbot-tui-grpc`
   - Adapters that format common-ui results for console display
   - Modernized command structure with no legacy commands
   - Tab completion support via rustyline
   - Can connect to local or remote servers

6. **Platform Architecture**:
   - Each platform has auth, client, and runtime modules
   - Platforms communicate via a central event bus
   - Twitch supports both IRC and EventSub simultaneously
   - VRChat includes OSC integration for avatar parameters and chatbox

7. **Plugin System**:
   - In-process plugins: Loaded as cdylib, implement specific traits
   - gRPC plugins: Connect via bidirectional streaming defined in plugin.proto
   - Capability-based permissions system

### Database

PostgreSQL with migrations in `migrations/`. The server auto-runs migrations on startup. Key tables:
- `users` with UUID primary keys
- `platform_identities` linking users to platform accounts
- `commands` and `redeems` for platform-specific features
- Credential storage with AES-GCM encryption

### Testing

Tests are in `maowbot-core/tests/`:
- Unit tests in `src/` files
- Integration tests in `tests/integration/`
- Most tests require PostgreSQL (can use `--test-threads=1` for database tests)

## Recent Updates (2025-06-04)

- **Command Structure**: Modernized TUI commands with consistent naming (ttv→twitch, plug→plugin)
- **New Commands**: Added credential, connection, diagnostics, system shutdown
- **Tab Completion**: Implemented unified completion system for all UI components
- **Config Management**: Added import/export functionality for configuration
- **Process Management**: Server and overlay can be controlled from TUI
- **Graceful Shutdown**: System shutdown command works regardless of how server was started

## Development Notes

- This is preproduction - APIs are unstable
- All crates use workspace dependencies defined in root Cargo.toml
- Platforms use different libraries: Twilight for Discord, twitch_api for Twitch Helix
- VRChat uses OSC via rosc and custom OSCQuery implementation
- Plugin development examples in `plugins/` directory
- Documentation for recent changes in `docs/` directory