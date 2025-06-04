# MaowBot Documentation

This directory contains technical documentation for the MaowBot project.

## Architecture & Design

- [UI_ARCHITECTURE_PROPOSAL.md](../UI_ARCHITECTURE_PROPOSAL.md) - Proposal for unified UI architecture
- [GRPC_MIGRATION.md](../GRPC_MIGRATION.md) - Migration guide from legacy to gRPC architecture

## Recent Development (2025-06-04)

- [TUI_REFACTORING_SUMMARY.md](TUI_REFACTORING_SUMMARY.md) - Summary of TUI command modernization
- [HELP_SYSTEM_UPDATE.md](HELP_SYSTEM_UPDATE.md) - Documentation of help system improvements
- [TAB_COMPLETION_PLAN.md](TAB_COMPLETION_PLAN.md) - Design for tab completion system
- [TAB_COMPLETION_IMPLEMENTATION.md](TAB_COMPLETION_IMPLEMENTATION.md) - Implementation details for TUI tab completion
- [UNIFIED_COMPLETION_DESIGN.md](UNIFIED_COMPLETION_DESIGN.md) - Architecture for unified completion across all UIs
- [UNIFIED_COMPLETION_SUMMARY.md](UNIFIED_COMPLETION_SUMMARY.md) - Summary of unified completion implementation
- [SHUTDOWN_IMPLEMENTATION.md](SHUTDOWN_IMPLEMENTATION.md) - Server shutdown command documentation

## Key Features

### Command System
The TUI has been modernized with a consistent command structure:
- Renamed commands for consistency (ttv→twitch, plug→plugin)
- Added new commands: credential, connection, diagnostics
- Consolidated related commands (user+member→user)
- Removed all legacy command references

### Tab Completion
A unified tab completion system supports:
- Context-aware completions (TUI, Twitch chat, Discord, overlay)
- Multiple providers (commands, emotes, users)
- Fuzzy matching with scoring
- Caching for performance

### Process Management
The TUI can now manage server and overlay processes:
- Start/stop server and overlay
- Monitor process status
- Graceful shutdown with configurable grace period

### Configuration
Enhanced configuration management:
- Import/export configurations
- List, get, set, delete operations
- Support for JSON/YAML formats

## Development Guidelines

See [CLAUDE.md](../CLAUDE.md) for AI assistant guidance and project overview.