// File: maowbot-tui/src/help/help_command.rs
/// Detailed help text for the "command" group:
///
///   - command list [platform]
///   - command setcooldown <commandName> <seconds> [platform]
///   - command setwarnonce <commandName> <true|false> [platform]
///   - command setrespond <commandName> <credentialId|username|none> [platform]
///   - command setplatform <commandName> <newPlatform> [oldPlatform]
///   - command enable <commandName> [platform]
///   - command disable <commandName> [platform]
///
/// This command manages the built-in or custom commands (chat triggers).
/// It allows viewing, enabling/disabling, adjusting cooldowns, changing the associated platform,
/// and specifying which credential (if any) should respond when triggered.
///
/// Examples:
///   command list
///   command list twitch-irc
///   command setcooldown !hello 5
///   command setwarnonce !shout true
///   command setrespond !test none twitch-irc
///   command setplatform !ping discord twitch-irc
///   command enable !mycommand
///   command disable mycommand
///
/// See below for details on each subcommand.

pub const COMMAND_HELP_TEXT: &str = r#"Command Management:

Subcommands:

  command list [platform]
    Lists all known commands. If a platform is given, only that platform’s commands are shown.
    Example: "command list twitch-irc"

  command setcooldown <commandName> <seconds> [platform]
    Sets the global cooldown (in seconds). During cooldown, re-use is blocked.
    Example: "command setcooldown !hello 5"

  command setwarnonce <commandName> <true|false> [platform]
    If set to true, the first user attempt during cooldown gets a warning. If false, no cooldown messages.

  command setrespond <commandName> <credentialId|username|none> [platform]
    Specifies the credential used for responding. E.g. if "myBotUser" is a known Twitch-IRC account,
    you can link the command to respond as that user. Use "none" to clear.

  command setplatform <commandName> <newPlatform> [oldPlatform]
    Moves a command from one platform to another. Defaults oldPlatform to "twitch-irc" if not specified.
    Example: "command setplatform !ping discord twitch-irc"

  command enable <commandName> [platform]
    Enables the specified command so it can be triggered.

  command disable <commandName> [platform]
    Disables the specified command.

Examples:
  command list
  command list twitch-irc
  command setcooldown !shout 10
  command setwarnonce !hello false
  command setrespond !roll kittyn twitch-irc
  command setplatform !ping vrchat twitch-irc
  command enable !newcmd
  command disable !spammycommand
"#;
