/// Detailed help text for connectivity-related commands:
///  - autostart
///  - start
///  - stop
///  - chat
pub const CONNECTIVITY_HELP_TEXT: &str = r#"Connectivity Commands:

  autostart <on/off> <platform> <account>
     Toggles whether a (platform, account) pair should auto-start on bot launch.
     If 'on', the pair is added to autostart config; if 'off', it's removed.

  start <platform> <account>
     Immediately starts the given platform runtime for that user account (connects it).

  stop <platform> <account>
     Immediately stops the given platform runtime for that user account (disconnects it).

  chat <on/off> [platform] [account]
     Controls whether chat messages are displayed in the TUI.
       - If you supply no platform/account, toggles for all.
       - If you supply platform only, toggles chat for that platform only.
       - If you supply both platform + account, toggles chat for that exact pair.

Examples:
  autostart on twitch MyGlobalUser
  start twitch MyGlobalUser
  stop twitch MyGlobalUser
  chat on twitch MyGlobalUser
"#;
