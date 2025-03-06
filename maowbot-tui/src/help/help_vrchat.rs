/// Detailed help text for the "vrchat" command and its subcommands.
pub const VRCHAT_HELP_TEXT: &str = r#"VRChat Command:
  This command integrates with the VRChat API to allow direct interaction with your VRChat experience
  from within the TUI. It enables you to query current world details, manage your avatar settings, and
  choose which VRChat account is active by default.

Subcommands:
  vrchat world [accountName]
      Retrieves and displays details about the VRChat world currently active for the registered VRChat account.
      If [accountName] is omitted, the bot uses the active account stored in configuration (defaulting to "broadcaster").
      Information provided includes:
        • World Name
        • Author Name
        • Last Updated timestamp
        • Date Published
        • Maximum Capacity

  vrchat avatar [accountName]
      Fetches and shows information about the current avatar associated with the registered VRChat account.
      Details include:
        • Avatar Name
        • Avatar ID

  vrchat avatar [accountName] change <avatarId>
      Sends a request to the VRChat API to update your avatar to the specified avatarId.
      This command initiates an avatar change process; please allow a moment for the update to take effect.

  vrchat instance [accountName]
      Fetches the user's current VRChat instance (world and instance details).

  vrchat account <accountName>
      Sets the default VRChat account for built-in commands (e.g. !world, !instance, !avatar).
      The specified accountName must correspond to a VRChat account registered within the bot's database.

Usage Examples:
  vrchat world
  vrchat avatar
  vrchat avatar change 1234567890abcdef
  vrchat instance
  vrchat account kittyn

Notes:
  • Ensure your VRChat credentials are correctly configured in the system before using these commands.
  • The [accountName] must correspond to a VRChat account registered within the bot's database.
  • After issuing an avatar change command, allow some time for the API to process the request.
  • For additional troubleshooting or advanced configuration, consult the VRChat API documentation.
"#;