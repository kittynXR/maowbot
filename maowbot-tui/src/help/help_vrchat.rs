/// Detailed help text for the "vrchat" command and its subcommands.
pub const VRCHAT_HELP_TEXT: &str = r#"VRChat Command:
  This command integrates with the VRChat API to allow direct interaction with your VRChat experience
  from within the TUI. It enables you to query current world details and manage your avatar settings.

Subcommands:
  vrchat world
      Retrieves and displays details about the VRChat world currently active for the registered VRChat account.
      Information provided includes:
        • World Name
        • Author Name
        • Last Updated timestamp
        • Date Published
        • Maximum Capacity

  vrchat avatar
      Fetches and shows information about the current avatar associated with the registered VRChat account.
      Details include:
        • Avatar Name
        • Avatar ID

  vrchat avatar change <avatarId>
      Sends a request to the VRChat API to update your avatar to the specified avatarId.
      This command initiates an avatar change process; please allow a moment for the update to take effect.

Usage Examples:
  vrchat world
  vrchat avatar
  vrchat avatar change 1234567890abcdef

Notes:
  • Ensure your VRChat credentials are correctly configured in the system before using these commands.
  • The <accountName> must correspond to a VRChat account registered within the bot's database.
  • After issuing an avatar change command, allow some time for the API to process the request.
  • For additional troubleshooting or advanced configuration, consult the VRChat API documentation.

"#;
