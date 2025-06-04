/// Detailed help text for the "credential" command (direct credential management).
pub const CREDENTIAL_HELP_TEXT: &str = r#"Credential Command:
  Direct management of platform credentials (OAuth tokens, API keys, etc.).

Subcommands:
  credential list [platform]
      Lists all stored credentials, optionally filtered by platform.
      Shows credential status (Active, Expired, etc.) and user roles.

  credential refresh <credential_id>
      Manually refreshes a specific credential using its refresh token.
      Use this when a credential is about to expire.

  credential revoke <credential_id> [--platform-revoke]
      Revokes a credential locally. With --platform-revoke, also
      revokes the token at the platform's OAuth endpoint.

  credential health [platform]
      Shows health statistics for credentials, including expiration
      status and refresh timestamps. Can filter by platform.

  credential batch-refresh <platform> [--force]
      Refreshes all credentials for a specific platform.
      Use --force to refresh even non-expired credentials.

Platforms:
  - twitch (or twitch-helix)
  - twitch-irc
  - twitch-eventsub
  - discord
  - vrchat

Examples:
  credential list
  credential list discord
  credential refresh 123e4567-e89b-12d3-a456-426614174000
  credential revoke 123e4567-e89b-12d3-a456-426614174000 --platform-revoke
  credential health
  credential batch-refresh twitch --force

Note: For adding new credentials, use the 'account add' command instead.
"#;