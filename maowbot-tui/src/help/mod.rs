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
pub mod help_osc;

fn show_general_help() -> String {
    let text = r#"MaowBot TUI - Available Commands:

Core Commands:
  help [command]         Show general help or detailed help for a command
  status [config]        Show system status (add 'config' to include settings)
  list                   List all known plugins
  quit                   Shut down the TUI

User Management:
  user                   Comprehensive user management (add, edit, search, roles, etc.)
  credential             Direct credential management (list, refresh, health)

Platform Management:
  platform               Manage platform configurations (add, remove, list)
  account                Manage platform accounts and credentials
  connection             Platform runtime control (start, stop, chat, autostart)

Content Management:
  command                Manage chat commands (cooldowns, responses, enable/disable)
  redeem                 Manage channel point redeems
  config                 Bot configuration (list, set, delete, export, import)

Platform-Specific:
  twitch                 Twitch-specific commands (join, part, message, etc.)
  vrchat                 VRChat integration (world, avatar, instance)
  drip                   VRChat avatar parameters and outfits
  osc                    OSC service control for VRChat parameters and chatbox

System & Development:
  plugin                 Plugin management (enable, disable, remove)
  ai                     AI provider configuration and chat
  diagnostics (diag)     System health monitoring and troubleshooting
  system                 Server and overlay process management
  test_harness           Testing framework for TUI functionality
  simulate               Trigger test events without going live

Type 'help <command>' for detailed information about any command.
"#;
    text.to_owned()
}

pub fn show_command_help(command: &str) -> String {
    match command {
        "" => show_general_help(),

        // Core Commands
        "status" => "Status Command:\n  Usage: status [config]\n    Shows system uptime and connected plugins.\n    Add 'config' to include bot_config entries.".to_owned(),
        "list" => "List Command:\n  Usage: list\n    Shows all known plugins (enabled or disabled).".to_owned(),
        "quit" => "Quit Command:\n  Usage: quit\n    Shuts down the TUI and the entire bot process.".to_owned(),

        // User Management
        "user" => help_unified_user::UNIFIED_USER_HELP_TEXT.to_owned(),
        "credential" => help_credential::CREDENTIAL_HELP_TEXT.to_owned(),

        // Platform Management
        "platform" => help_platform::PLATFORM_HELP_TEXT.to_owned(),
        "account" => help_account::ACCOUNT_HELP_TEXT.to_owned(),
        "connection" => help_connection::CONNECTION_HELP_TEXT.to_owned(),

        // Content Management
        "command" => help_command::COMMAND_HELP_TEXT.to_owned(),
        "redeem" => help_redeem::REDEEM_HELP_TEXT.to_owned(),
        "config" => help_config::CONFIG_HELP_TEXT.to_owned(),

        // Platform-Specific
        "twitch" => help_twitch::TWITCH_HELP_TEXT.to_owned(),
        "discord" => "Discord commands are not yet fully implemented via gRPC.".to_owned(),
        "vrchat" => help_vrchat::VRCHAT_HELP_TEXT.to_owned(),
        "drip" => help_drip::DRIP_HELP_TEXT.to_owned(),
        "osc" => help_osc::OSC_HELP_TEXT.to_owned(),

        // System & Development
        "plugin" => help_plugin::PLUGIN_HELP_TEXT.to_owned(),
        "ai" => help_ai::AI_HELP_TEXT.to_owned(),
        "diagnostics" | "diag" => help_diagnostics::DIAGNOSTICS_HELP_TEXT.to_owned(),
        "system" => help_system::system_help().to_owned(),
        "test_harness" => help_test_harness::help_test_harness(),
        "simulate" => help_simulate::help_simulate(),

        // Legacy redirects
        "member" => "The 'member' command has been merged into 'user'.\nUse 'help user' for details.".to_owned(),
        "autostart" | "start" | "stop" | "chat" => "These commands have been merged into 'connection'.\nUse 'help connection' for details.".to_owned(),
        "ttv" => "The 'ttv' command has been renamed to 'twitch'.\nUse 'help twitch' for details.".to_owned(),
        "plug" => "The 'plug' command has been renamed to 'plugin'.\nUse 'help plugin' for details.".to_owned(),

        // fallback
        other => format!("No detailed help found for '{}'. Type 'help' for an overview.", other),
    }
}
