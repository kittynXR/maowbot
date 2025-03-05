/// Detailed help text for the "command" command group:
///
///   - command list [platform]
///   - command setcooldown <commandName> <seconds> [platform]
///   - command setwarnonce <commandName> <true|false> [platform]
///   - command setrespond <commandName> <credentialId|username|none> [platform]
///   - command enable <commandName> [platform]
///   - command disable <commandName> [platform]
///
/// This command manages the built-in or custom commands (e.g. chat commands).
/// It allows viewing, enabling/disabling, adjusting cooldowns, and specifying
/// which credential (if any) should respond when the command is triggered.
///
/// Usage Examples:
///   command list
///   command list twitch-irc
///   command setcooldown !hello 5
///   command setwarnonce !shout true
///   command setrespond !test none twitch-irc
///   command enable !mycommand
///   command disable mycommand
///
/// See below for details on each subcommand.

pub const COMMAND_HELP_TEXT: &str = r#"Command Management:

Subcommands:
  command list [platform]
     Lists all known commands for the given platform. Defaults to "twitch-irc" if none specified.

  command setcooldown <commandName> <seconds> [platform]
     Sets the cooldown (in seconds) for the given command. During cooldown, re-use is blocked.
     Example: command setcooldown !hello 5

  command setwarnonce <commandName> <true|false> [platform]
     If set to true, the first user attempt during cooldown gets a warning message. If false, no message is sent on cooldown.

  command setrespond <commandName> <credentialId|username|none> [platform]
     Specifies the credential used for responding. For example, if "myBotUser" is a known user with a Twitch-IRC credential,
     you can link the command to that credential so it responds as that user. Use "none" to clear.

  command enable <commandName> [platform]
     Enables the specified command so it can be triggered.

  command disable <commandName> [platform]
     Disables the specified command, preventing it from triggering.

Examples:
  command list
  command list twitch-irc
  command setcooldown !shout 10
  command setwarnonce !hello false
  command setrespond !roll kittyn twitch-irc
  command enable !newcmd
  command disable !spammycommand
"#;
