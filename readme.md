MaowBot (Preproduction / Greenfield)

MaowBot is an experimental, multi-platform bot and plugin framework that’s still in preproduction. Expect frequent changes and incomplete features while the core API stabilizes. The project primarily targets chat integrations (Twitch, Discord, VRChat, etc.) with an optional TUI (text-based UI) and support for both in-process and gRPC-based plugins.
Repository Overview

This repo is organized as a Cargo workspace, with several crates and folders:

```bash
kittynxr-maowbot/
├── Cargo.toml             # Workspace definitions (all crates, shared dependencies)
├── maowbot-core/          # The core library with main bot logic, platform integrations, DB code
├── maowbot-proto/         # Protobuf/gRPC definitions, compiled via tonic_build
├── maowbot-server/        # The CLI server that runs the bot, orchestrates platforms/plugins
├── maowbot-tui/           # Optional text-based UI for local administration
├── migrations/            # SQL migrations (Postgres schema)
└── plugins/               # Example plugin crates (both in-process and gRPC-based)
```

Key Components

    maowbot-core/
        Houses most core logic:
            platforms/platforms/ for each platform integration (Twitch, Discord, VRChat, etc.)
            auth/auth/ for authentication flows
            services/services/ for message and user management
            repositories/repositories/ for Postgres database abstraction
            plugins/plugins/ a management layer for plugin integration
            tasks/tasks/ recurring tasks, maintenance/credential refresh jobs

    maowbot-proto/
        Defines gRPC service contracts in proto/plugin.proto and generates Rust stubs.
        Used by the server and any remote plugin that speaks gRPC.

    maowbot-server/
        A CLI application for running the bot.
        Spins up the bot’s event bus, loads plugins, starts the main gRPC server for remote plugins, and manages platform connections.

    maowbot-tui/
        An optional text-based console for local administration.
        Provides commands to enable/disable plugins, add credentials, view status, etc.

    migrations/
        SQL migrations for setting up the bot’s Postgres tables.
        The server automatically runs these on startup (db.migrate()).

    plugins/
        Example plugin crates that demonstrate:
            gRPC plugins (e.g., plugin_hello/), running out-of-process.
            In-process plugins (e.g., tui_plugin/), compiled as cdylib and loaded dynamically.
        Illustrates how to integrate with the maowbot-proto gRPC schema or with the in-process API.

Status & Notes

    Preproduction / Greenfield: The API is not stable yet; breaking changes can happen.
    Rust Workspace: Each sub-crate can be built/tested independently (cargo build -p maowbot-core etc.).
    Postgres: Most persistence is tested only on Postgres. The migrations/ folder has the initial schema.

Getting Started

    Install Rust (preferably via rustup).
    Start Postgres (the server crate can optionally spawn a local Postgres for dev use).
    Run cargo build --all to compile every crate and plugin.
    Explore maowbot-server:

    cd maowbot-server
    cargo run -- --help

For plugin development, see plugins/plugin_hello (a gRPC example) or plugins/tui_plugin (an in-process example).

Disclaimer: Because this is preproduction, some features may be incomplete or subject to redesign. Pull requests and experiments are welcome!
