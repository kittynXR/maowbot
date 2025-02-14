/// Detailed help text for the "plug" command (plugin management).
pub const PLUGIN_HELP_TEXT: &str = r#"Plugin Command:
  Manages loaded plugins (either gRPC-based or in-process .so/.dll).

Subcommands:
  plug enable <pluginName>
      Enables the plugin if it’s disabled. If it was never loaded
      but is a known dynamic-lib plugin, attempts to load it.

  plug disable <pluginName>
      Disables the plugin if it’s enabled. If it’s an in-process plugin,
      unloads it from memory. gRPC plugins remain connected but flagged disabled.

  plug remove <pluginName>
      Removes the plugin record entirely from the system. If it’s in memory,
      unloads/stops it. Also removes from the persisted JSON state so it
      won’t reload on next startup.

Examples:
  plug enable MyPlugin
  plug disable MyPlugin
  plug remove MyPlugin
"#;
