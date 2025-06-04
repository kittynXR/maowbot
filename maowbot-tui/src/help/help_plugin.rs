/// Detailed help text for the "plugin" command (plugin management).
pub const PLUGIN_HELP_TEXT: &str = r#"Plugin Command:
  Manages loaded plugins (either gRPC-based or in-process .so/.dll).

Subcommands:
  plugin enable <pluginName>
      Enables the plugin if it’s disabled. If it was never loaded
      but is a known dynamic-lib plugin, attempts to load it.

  plugin disable <pluginName>
      Disables the plugin if it’s enabled. If it’s an in-process plugin,
      unloads it from memory. gRPC plugins remain connected but flagged disabled.

  plugin remove <pluginName>
      Removes the plugin record entirely from the system. If it’s in memory,
      unloads/stops it. Also removes from the persisted JSON state so it
      won’t reload on next startup.

Examples:
  plugin enable MyPlugin
  plugin disable MyPlugin
  plugin remove MyPlugin
"#;
