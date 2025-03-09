//! Central help module that provides a single entry point (`show_command_help`)
//! to display usage or subcommand details for any recognized TUI command.

pub mod help_account;
pub mod help_connectivity;
pub mod help_member;
pub mod help_platform;
pub mod help_plugin;
pub mod help_ttv;
pub mod help_user;
pub mod help_vrchat;
pub mod help_command;
pub mod help_redeem;

fn show_general_help() -> String {
    let text = r#"MaowBot TUI - Available Commands:

  help [command]
    Show general help, or detailed help on a specific command.

  list
    Lists all known plugins by name.

  status [config]
    Shows current uptime + connected plugins, etc.
    'status config' includes bot_config key/values.

  plug <enable|disable|remove> <pluginName>
    Manage plugin connections or remove them from the system.

  platform <add|remove|list|show> ...
    Add, remove, or inspect platform configurations.

  account <add|remove|list|show|refresh> ...
    Manage user credentials for a given platform.

  user <add|remove|edit|info|search|list> ...
    Manage user records in the database.

  member <info|chat|list|search|note|merge|roles> ...
    Manage members (extended user data, merges, roles, chat logs).

  command <list|setcooldown|setwarnonce|setrespond|enable|disable>
    Manage built-in or custom commands, including cooldowns and response credentials.

  autostart <on/off> <platform> <account>
    Toggle a (platform,account) autostart on boot.

  start <platform> [account]
    Connect a platform runtime for the given user account.

  stop <platform> [account]
    Disconnect a platform runtime for the given user account.

  chat <on/off> [platform] [account]
    Enable or disable chat display in TUI (with optional filters).

  ttv <active|join|part|msg|chat|default> ...
    Twitch IRC commands, e.g. 'ttv join #channel', etc.

  vrchat <world|avatar|instance> ...
    VRChat integration commands.

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
        "autostart" | "start" | "stop" | "chat" => help_connectivity::CONNECTIVITY_HELP_TEXT.to_owned(),
        "platform" => help_platform::PLATFORM_HELP_TEXT.to_owned(),
        "plug" => help_plugin::PLUGIN_HELP_TEXT.to_owned(),
        "user" => help_user::USER_HELP_TEXT.to_owned(),
        "member" => help_member::MEMBER_HELP_TEXT.to_owned(),
        "ttv" => help_ttv::TTV_HELP_TEXT.to_owned(),
        "vrchat" => help_vrchat::VRCHAT_HELP_TEXT.to_owned(),

        // NEW: "command" => show help_command
        "command" => help_command::COMMAND_HELP_TEXT.to_owned(),

        // "list" built-in help snippet:
        "list" => {
            r#"List Command:
  Usage: list
    Shows all known plugins (enabled or disabled).
"#
                .to_owned()
        },
        // "status" built-in help snippet:
        "status" => {
            r#"Status Command:
  Usage: status [config]
    - status: shows uptime + connected plugin list
    - status config: also shows all bot_config entries
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

        // fallback if not recognized
        other => format!("No detailed help found for '{}'. Type 'help' for an overview.", other),
    }
}