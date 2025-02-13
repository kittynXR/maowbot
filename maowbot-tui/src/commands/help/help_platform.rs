/// Detailed help text for the "platform" command (managing OAuth client_id/secret, etc.).
pub const PLATFORM_HELP_TEXT: &str = r#"Platform Command:
  Manages a platform config record in the DB (client_id, client_secret, etc.).

Subcommands:
  platform add <platformName>
      Prompts for client_id and client_secret if needed, then stores them.
      If <platformName> is "twitch", we also store "twitch-irc" and "twitch-eventsub"
      configurations in one go, reusing the same client_id/client_secret.

  platform remove <platformName>
      Removes the DB record for that platform's config. Also removes "twitch-irc" and
      "twitch-eventsub" if the main <platformName> was "twitch".

  platform list [platformName]
      Lists all known platform configs. If a name is given, filters to just that one.

  platform show <platformName>
      Shows detailed info about the DB record for that platform config.

Usage Examples:
  platform add twitch
  platform remove twitch
  platform list
  platform show discord
"#;
