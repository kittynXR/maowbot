// File: maowbot-tui/src/help/help_config.rs
//
// Detailed help text for the "config" command group.

pub const CONFIG_HELP_TEXT: &str = r#"Config Command:
  Manage bot configuration values with import/export support.

Usage:

  config
    Shows usage for the config command (this text).

  config list  (or: config l)
    Lists all key-value pairs from the bot_config table.

  config get <key>  (or: config g <key>)
    Gets the value for a specific key.

  config set <key> <value>  (or: config s <key> <value>)
    Inserts or updates the given key with the provided value.

  config delete <key>  (or: config d <key>)
    Removes the specified key (and its value) from the bot_config table.

  config export [filename]
    Exports all configuration to a JSON file.
    Default filename: bot_config_export.json

  config import <filename> [--merge]
    Imports configuration from a JSON file.
    Without --merge: Replaces all existing configs.
    With --merge: Only adds new keys, preserves existing ones.

Examples:
  config l
  config g callback_port
  config s ttv_broadcaster_channel #mychannel
  config delete callback_port
  config export my_config_backup.json
  config import my_config_backup.json
  config import new_settings.json --merge

Export File Format:
  {
    "version": "1.0",
    "exported_at": "2024-01-01T12:00:00Z",
    "configs": {
      "key1": "value1",
      "key2": "value2"
    }
  }
"#;
