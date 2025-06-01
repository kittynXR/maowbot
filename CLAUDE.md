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

2. **maowbot-proto**: gRPC definitions for remote plugins using bidirectional streaming

3. **maowbot-server**: CLI application that orchestrates everything - runs migrations, starts platforms, loads plugins

4. **Platform Architecture**:
   - Each platform has auth, client, and runtime modules
   - Platforms communicate via a central event bus
   - Twitch supports both IRC and EventSub simultaneously
   - VRChat includes OSC integration for avatar parameters and chatbox

5. **Plugin System**:
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

## Development Notes

- This is preproduction - APIs are unstable
- All crates use workspace dependencies defined in root Cargo.toml
- Platforms use different libraries: Twilight for Discord, twitch_api for Twitch Helix
- VRChat uses OSC via rosc and custom OSCQuery implementation
- Plugin development examples in `plugins/` directory