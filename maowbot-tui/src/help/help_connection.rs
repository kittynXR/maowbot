/// Detailed help text for the "connection" command (unified connectivity management).
pub const CONNECTION_HELP_TEXT: &str = r#"Connection Command:
  Manages platform connections and chat display settings.

Subcommands:
  connection start <platform> [account]
      Connects to the specified platform. If account is not specified,
      uses the default or first available account for that platform.

  connection stop <platform> [account]
      Disconnects from the specified platform. If account is not specified,
      stops all connections for that platform.

  connection status
      Shows the current connection status for all platforms and accounts.

  connection autostart on <platform> <account>
      Enables automatic connection on bot startup for the specified
      platform and account combination.

  connection autostart off <platform> <account>
      Disables automatic connection on bot startup.

  connection autostart list
      Lists all autostart configurations and their status.

  connection chat on [platform] [account]
      Enables chat display in the TUI. Can be filtered by platform
      and/or account. Without filters, shows all chat messages.

  connection chat off
      Disables all chat display in the TUI.

Examples:
  connection start twitch kittyn
  connection stop discord
  connection status
  connection autostart on twitch-irc kittyn
  connection autostart list
  connection chat on twitch
  connection chat off

Note: The legacy commands 'start', 'stop', 'autostart', and 'chat' 
      are still supported but using 'connection' is recommended.
"#;