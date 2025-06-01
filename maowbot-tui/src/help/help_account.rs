/// Detailed help text for the "account" command and its subcommands.
pub const ACCOUNT_HELP_TEXT: &str = r#"Account Command:
  Manages user credentials for a given platform.

Subcommands:
  account add <platform> <desired_global_username>
      1. Prompts if it's a bot account or not
      2. Finds or creates a DB user with the given global username
      3. Begins the auth flow (OAuth or API key, etc.)
      4. Stores the resulting credentials for that user+platform

  account remove <platform> <usernameOrUUID>
      Revokes and removes stored credentials for the given user on a platform.
      <usernameOrUUID> can be either the user's name (global_username) or their DB UUID.

  account list [platform]
      Lists all stored platform credentials. If a platform is given, filters to just that platform.

  account show <platform> <usernameOrUUID>
      Shows detailed info about the credential record (tokens, expiration, etc.).
      <usernameOrUUID> can be either the user's name or their DB UUID.

Usage Examples:
  account add twitch testUser
  account remove discord testUser
  account list twitch
  account show twitch testUser
"#;
