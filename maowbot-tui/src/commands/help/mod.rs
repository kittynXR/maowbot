//! Central help module that provides a single entry point (`show_command_help`)
//! to display usage or subcommand details for any recognized TUI command.

pub mod help_account;
pub mod help_connectivity;
pub mod help_platform;
pub mod help_plugin;
pub mod help_user;

/// If the user types just `help` (with no subcommand), show a general usage overview.
fn show_general_help() -> String {
    let text = r#"MaowBot TUI - Available Commands:

  help [command]
    Show general help, or detailed help on a specific command.

  list
    Lists all known (loaded or not) plugins by name.

  status [config]
    Shows current uptime, connected plugins, etc.
    Use "status config" to list all bot_config key/values.

  plug <enable|disable|remove> <pluginName>
    Manage plugin connections or remove them from the system.

  platform <add|remove|list|show> ...
    Add, remove, or inspect platform configurations (client_id, secret).

  account <add|remove|list|show> ...
    Manage user credentials for a given platform.

  user <add|remove|edit|info|search> ...
    Manage user records in the database.

  autostart <on/off> <platform> <account>
    Toggle whether a certain platform+account pair autostarts on boot.

  start <platform> <account>
    Start (connect) a platform runtime for a given user account.

  stop <platform> <account>
    Stop (disconnect) a platform runtime for a given user account.

  chat <on/off> [platform] [account]
    Enable/disable live chat display in the TUI, optionally filtered.

  quit
    Shut down the TUI (and the whole bot system).
"#;
    text.to_owned()
}

/// Show help for a specific command. If unknown, we just return a short message.
pub fn show_command_help(command: &str) -> String {
    match command {
        "" => show_general_help(),

        "account" => help_account::ACCOUNT_HELP_TEXT.to_owned(),
        "autostart" | "start" | "stop" | "chat" => help_connectivity::CONNECTIVITY_HELP_TEXT.to_owned(),
        "platform" => help_platform::PLATFORM_HELP_TEXT.to_owned(),
        "plug" => help_plugin::PLUGIN_HELP_TEXT.to_owned(),
        "user" => help_user::USER_HELP_TEXT.to_owned(),

        // fallback for recognized top-level commands that don't have big subcommands:
        "list" => {
            r#"List Command:
  Usage: list
    Shows all known plugins (whether enabled, disabled, or not loaded).
"#
                .to_owned()
        },
        "status" => {
            r#"Status Command:
  Usage: status [config]
    - status: shows uptime + connected plugin list
    - status config: also shows all key-value pairs in bot_config
"#
                .to_owned()
        },
        "quit" => {
            r#"Quit Command:
  Usage: quit
    Shuts down the TUI and the entire bot process.
"#
                .to_owned()
        },

        // If not recognized
        other => format!("No detailed help found for '{}'. Type 'help' for an overview.", other),
    }
}