// File: maowbot-tui/src/help/help_config.rs
//
// Detailed help text for the "config" command group.

pub const CONFIG_HELP_TEXT: &str = r#"Config Command:

Usage:

  config
    Shows usage for the config command (this text).

  config list  (or: config l)
    Lists all key-value pairs from the bot_config table.

  config set <key> <value>  (or: config s <key> <value>)
    Inserts or updates the given key with the provided value.

  config delete <key>       (or: config d <key>)
    Removes the specified key (and its value) from the bot_config table.

Examples:
  config l
  config s ttv_broadcaster_channel #mychannel
  config delete callback_port
"#;
