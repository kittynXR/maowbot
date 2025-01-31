# MaowBot - KittynXR/maowbot

> **Status**: **Under active development** – no stable or working end-product at this time.

This repository aims to build a multi-platform chatbot and plugin system in Rust, intended to interface with platforms like Twitch, Discord, VRChat, etc. The architecture is structured around a modular design, with separate modules handling authentication, database operations, plugin management, event broadcasting, and more.

---

## Overview

**MaowBot** is composed of the following major parts:

1. **Main Application** (`src/main.rs`):
    - Implements the command-line interface (CLI) via `clap`.
    - Initializes the database, plugin manager, event bus, and platform runtimes.
    - Sets up gRPC server (using Tonic) for plugins to connect.
    - Runs tasks like monthly maintenance, credential refresh, etc.

2. **EventBus** (`src/eventbus`):
    - A simple publish/subscribe system that broadcasts events (e.g., incoming chat messages) to any subscribers.
    - Subscribers include the database logger (batch-writing chat messages), plugin manager (to forward events to plugins), etc.

3. **Plugin System** (`src/plugins`):
    - Defines a trait-based mechanism for both in-process and gRPC-based plugins.
    - `plugin_service` (gRPC) allows external plugins to stream messages in/out (see `proto/plugin.proto`).
    - `PluginManager` coordinates active plugins, verifies passphrases, and routes messages/capabilities.

4. **Platform Integrations** (`src/platforms`):
    - Contains submodules for Twitch, Discord, VRChat, etc., each implementing a `PlatformIntegration` trait.
    - Manages how we connect/observe chat events, authenticate, and send messages to each platform.

5. **Database and Repositories** (`src/db`, `src/repositories`):
    - Uses **sqlx** with SQLite.
    - `migrations/` folder has incremental .sql scripts for schema changes.
    - `src/repositories/sqlite/*` define data-access layers, e.g. storing credentials, user profiles, chat logs, etc.

6. **Services** (`src/services`):
    - Logic for handling user creation/lookup (`UserService`) and message processing (`MessageService`).
    - These services use the event bus for logging and the repositories for data persistence.

7. **Auth** (`src/auth`):
    - Central `AuthManager` and `UserManager` logic.
    - Delegates platform-specific authentication (OAuth2, tokens, etc.) to each platform’s authenticator module.
    - Stores encrypted credentials in the DB using a cryptography helper (`src/crypto`).

8. **Tasks** (`src/tasks`)
    - Longer-running or scheduled tasks:
        - `monthly_maintenance` for archiving old chat data and generating user analysis summaries.
        - `credential_refresh` for scanning and refreshing expiring tokens.

---

## Current Features & Limitations

- **Multiple Platforms**: Code stubs exist for Twitch, Discord, VRChat integrations.
- **In-memory + SQLite**: Testing in memory (`:memory:`) or local file-based SQLite DB.
- **Plugin System**:
    - Supports dynamic (in-process) or gRPC-based external plugins.
    - Plugin capabilities (like `SendChat`, `ReceiveChatEvents`, etc.) are requested and granted with some basic policy.
- **EventBus**: Central place for all inbound/outbound chat messages, logging tasks, etc.
- **Data Model**:
    - Users, platform identities, platform credentials, chat messages, analytics, user analysis, etc.
    - Migrations in `./migrations/` keep the DB schema up to date.

> **Important**: This codebase is in **heavy development**. Many methods and modules are stubs or incomplete. There is **no guaranteed working end-product** at this time.

---

## Architecture Diagram (Text-Based)

Below is a high-level text diagram showing how the core components interact:

               +------------------+
               |   Plugins (gRPC) |
               +-------+----------+
                       |
                       | (bidirectional gRPC)
                       v
                +----------------------+
                |  Plugin Manager     |
                | (manages plugins)   |
                +----------------------+
                       |
                       | publishes / subscribes
                       v
                +----------------------+
                |     EventBus        |
                |(publish/subscribe   |
                |  for all events)    |
                +----------------------+
                  /        |        \
                 /         |         \
                v          v          v
         +------------+  +-------------+  +--------------+
         |  Services  |  |  Platforms  |  |   DB Logger  |
         | (UserSvc,  |  |(Twitch, etc)|  |(Batched Chat |
         | MsgSvc)    |  +-------------+  |   Storage)   |
         +------------+                   +--------------+
                 |
                 |   DB Queries (Repositories)
                 v
            +---------------+
            |   SQLite DB   |
            +---------------+


- **Plugins** connect over gRPC or in-process.
- **Plugin Manager** is the central aggregator for plugin traffic, working with the `EventBus`.
- **EventBus** broadcasts chat messages and system events.
- **Services** handle business logic (creating users, caching messages, etc.) and also talk to the DB via Repositories.
- **DB Logger** is a subscriber that batches chat messages to store in the SQLite DB.
- **Platforms** represent external integrations like Twitch or Discord, producing chat events that also flow into the `EventBus`.

---

## Building & Running

Since this is still under development:

1. **Install Rust** (1.60+ or 2021 edition recommended).
2. **Clone** this repository.
3. **Compile** using Cargo:
   ```bash
   cd kittynxr-maowbot
   cargo build
4. **Run** (server mode or client mode):
   ```bash
   cargo run -- --mode=server
   # or
   cargo run -- --mode=client

Note: If you want to run the gRPC-based plugin example, see plugin_hello/ for a small sample plugin using Tonic.

## Directory Structure

Below is an abbreviated listing of the key files and folders:
 
    Directory structure:
    └── kittynxr-maowbot/
        ├── Cargo.toml
        ├── build.rs
        ├── migrations/
        │   ├── 20241216000000_initial_schema.sql
        │   ├── 20241216000001_add_platform_identities.sql
        │   ├── 20241218000000_add_platform_credentials.sql
        │   ├── 20250121000000_add_analytics_tables.sql
        │   ├── 20250121000001_add_username_and_link_tables.sql
        │   ├── 20250127000000_add_user_analysis.sql
        │   └── 20250128000000_add_user_analysis_history.sql
        ├── plugin_hello/
        │   ├── Cargo.toml
        │   └── src/
        │       └── main.rs
        ├── proto/
        │   └── plugin.proto
        ├── src/
        │   ├── error.rs
        │   ├── http.rs
        │   ├── lib.rs
        │   ├── main.rs
        │   ├── auth/
        │   │   ├── manager.rs
        │   │   ├── mod.rs
        │   │   ├── user_manager.rs
        │   │   └── user_manager_tests.rs
        │   ├── cache/
        │   │   ├── message_cache.rs
        │   │   └── mod.rs
        │   ├── crypto/
        │   │   └── mod.rs
        │   ├── db/
        │   │   └── mod.rs
        │   ├── eventbus/
        │   │   ├── db_logger.rs
        │   │   └── mod.rs
        │   ├── models/
        │   │   ├── mod.rs
        │   │   └── user_analysis.rs
        │   ├── platforms/
        │   │   ├── manager.rs
        │   │   ├── mod.rs
        │   │   ├── discord/
        │   │   │   ├── auth.rs
        │   │   │   ├── mod.rs
        │   │   │   └── runtime.rs
        │   │   ├── twitch/
        │   │   │   ├── auth.rs
        │   │   │   ├── mod.rs
        │   │   │   └── runtime.rs
        │   │   └── vrchat/
        │   │       ├── auth.rs
        │   │       ├── mod.rs
        │   │       └── runtime.rs
        │   ├── plugins/
        │   │   ├── manager.rs
        │   │   ├── mod.rs
        │   │   ├── tui_plugin.rs
        │   │   └── proto/
        │   │       └── mod.rs
        │   ├── repositories/
        │   │   ├── mod.rs
        │   │   └── sqlite/
        │   │       ├── analytics.rs
        │   │       ├── credentials.rs
        │   │       ├── link_requests.rs
        │   │       ├── mod.rs
        │   │       ├── platform_identity.rs
        │   │       ├── user.rs
        │   │       ├── user_analysis.rs
        │   │       └── user_audit_log.rs
        │   ├── services/
        │   │   ├── message_service.rs
        │   │   ├── mod.rs
        │   │   └── user_service.rs
        │   └── tasks/
        │       ├── credential_refresh.rs
        │       ├── mod.rs
        │       └── monthly_maintenance.rs
        └── tests/
            ├── auth_tests.rs
            ├── cache_tests.rs
            ├── credential_tests.rs
            ├── db_tests.rs
            ├── eventbus_tests.rs
            ├── monthly_maintenance_tests.rs
            ├── platform_tests.rs
            ├── plugin_manager_tests.rs
            ├── plugin_services_grpc_tests.rs
            ├── repository_tests.rs
            ├── services_tests.rs
            ├── shutdown_tests.rs
            └── twitch_runtime_tests.rs

## Testing

* There are various integration tests in tests/*.rs.
* Many tests use an in-memory SQLite database (:memory:), applying migrations automatically.
* Run all tests with:

   ```bash
   
   cargo test
  
## License

License: TBD (Not determined yet).
We have placeholders for open-source usage, but no official license has been chosen. Use at your own risk until a license is finalized.

## Contributing

As the project is still in early development, contributions, bug reports, and feature ideas are welcome. However, please note that stability and working features are not guaranteed at this stage.

© 2025+ – Under development by the contributors. No official release yet.