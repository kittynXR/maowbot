//! Central help module that provides a single entry point (`show_command_help`)
//! to display usage or subcommand details for any recognized TUI command.

pub mod help_account;
pub mod help_ai;
pub mod help_connectivity;
pub mod help_member;
pub mod help_platform;
pub mod help_plugin;
pub mod help_twitch;
pub mod help_user;
pub mod help_vrchat;
pub mod help_command;
pub mod help_redeem;
pub mod help_credential;
pub mod help_connection;
pub mod help_unified_user;
pub mod help_diagnostics;

// NEW:
pub mod help_config;
pub mod help_drip;
pub mod help_test_harness;
pub mod help_simulate;
pub mod help_system;

fn show_general_help() -> String {
    let text = r#"MaowBot TUI - Available Commands:

  help [command]
    Show general help, or detailed help on a specific command.

  list
    Lists all known plugins by name.

  status [config]
    Shows current uptime + connected plugins, etc.
    'status config' includes bot_config key/values.

  plugin <enable|disable|remove> <pluginName>
    Manage plugin connections or remove them from the system.

  platform <add|remove|list|show> ...
    Add, remove, or inspect platform configurations.

  account <add|remove|list|show|refresh> ...
    Manage user credentials for a given platform.
  
  credential <list|refresh|revoke|health|batch-refresh> ...
    Direct credential management (tokens, expiration, health).

  user <add|remove|edit|info|search|list|chat|note|merge|roles|analysis> ...
    Comprehensive user management (includes all former 'member' functionality).
    
  member [deprecated]
    This command has been merged into 'user'. Use 'user' for all functionality.

  command <list|setcooldown|setwarnonce|setrespond|enable|disable>
    Manage built-in or custom commands, including cooldowns and response credentials.

  redeem <list|create|delete|cost|enable|disable|...>
    Manage channel point redeems.

  config <list|set|delete>
    Manage key-value pairs in the bot_config table.

  connection <start|stop|autostart|chat|status> ...
    Unified connection management for platforms (start, stop, autostart, chat).
  
  autostart <on/off> <platform> <account>
    Toggle a (platform,account) autostart on boot. (Legacy - use 'connection')

  start <platform> [account]
    Connect a platform runtime for the given user account. (Legacy - use 'connection')

  stop <platform> [account]
    Disconnect a platform runtime for the given user account. (Legacy - use 'connection')

  chat <on/off> [platform] [account]
    Enable or disable chat display in TUI (with optional filters). (Legacy - use 'connection')

  twitch <active|join|part|msg|chat|default> ...
    Twitch IRC commands, e.g. 'twitch join #channel', etc.

  vrchat <world|avatar|instance> ...
    VRChat integration commands.

  drip <set|list|fit|props> ...
    Manage VRChat avatar parameters (props, fits, etc.) in the Drip system.

  ai <enable|disable|status|openai|anthropic|chat|addtrigger|removetrigger|listtriggers|systemprompt>
    Configure and interact with AI providers for the chat bot.

  test_harness <run-all|twitch|commands|redeems|grpc>
    Run the test harness for testing TUI functionality.

  simulate <type> [args...]
    Trigger simulated Twitch events for testing without being live.

  system <server|overlay> [start|stop|status]
    Manage the MaowBot server and overlay processes.
  
  diagnostics <health|status|metrics|logs|test>
    System health monitoring and troubleshooting (alias: diag).

  quit
    Shut down the TUI (and the entire bot).
"#;
    text.to_owned()
}

pub fn show_command_help(command: &str) -> String {
    match command {
        "" => show_general_help(),

        // Existing help lookups:
        "account" => help_account::ACCOUNT_HELP_TEXT.to_owned(),
        "ai" => help_ai::AI_HELP_TEXT.to_owned(),
        "autostart" | "start" | "stop" | "chat" => help_connectivity::CONNECTIVITY_HELP_TEXT.to_owned(),
        "platform" => help_platform::PLATFORM_HELP_TEXT.to_owned(),
        "plugin" => help_plugin::PLUGIN_HELP_TEXT.to_owned(),
        "user" => help_unified_user::UNIFIED_USER_HELP_TEXT.to_owned(),
        "member" => "The 'member' command has been deprecated and merged into 'user'.\nUse 'help user' to see all available functionality.".to_owned(),
        "twitch" => help_twitch::TWITCH_HELP_TEXT.to_owned(),
        "vrchat" => help_vrchat::VRCHAT_HELP_TEXT.to_owned(),
        "command" => help_command::COMMAND_HELP_TEXT.to_owned(),
        "redeem" => help_redeem::REDEEM_HELP_TEXT.to_owned(),
        "credential" => help_credential::CREDENTIAL_HELP_TEXT.to_owned(),
        "connection" => help_connection::CONNECTION_HELP_TEXT.to_owned(),

        // NEW:
        "config" => help_config::CONFIG_HELP_TEXT.to_owned(),
        "drip" => help_drip::DRIP_HELP_TEXT.to_owned(),
        "test_harness" => help_test_harness::help_test_harness(),
        "simulate" => help_simulate::help_simulate(),
        "system" => help_system::system_help().to_owned(),
        "diagnostics" | "diag" => help_diagnostics::DIAGNOSTICS_HELP_TEXT.to_owned(),

        // Built-in help snippet for "list"
        "list" => {
            r#"List Command:
  Usage: list
    Shows all known plugins (enabled or disabled).
"#
                .to_owned()
        },

        // Built-in help snippet for "status"
        "status" => {
            r#"Status Command:
  Usage: status [config]
    - status: shows uptime + connected plugin list
    - status config: also shows all bot_config entries
"#
                .to_owned()
        },

        // Built-in help snippet for "quit"
        "quit" => {
            r#"Quit Command:
  Usage: quit
    Shuts down the TUI and the entire bot process.
"#
                .to_owned()
        },

        // fallback
        other => format!("No detailed help found for '{}'. Type 'help' for an overview.", other),
    }
}
