/// Detailed help text for the unified "user" command (combines user + member functionality)
pub const UNIFIED_USER_HELP_TEXT: &str = r#"User Command:
  Comprehensive user management including profiles, analysis, and platform identities.

Basic Operations:
  user add <username>
      Creates a new user with the specified username.

  user remove <usernameOrUUID>
      Removes a user (soft delete by default).

  user edit <usernameOrUUID>
      Interactively edit user properties (active status).

  user info <usernameOrUUID>
      Shows detailed user information including platform identities and analysis.

  user list [pageSize] [pageNum]
      Lists all users with pagination (default: 20 per page).

  user search <query>
      Searches for users by username or UUID.

Extended Operations:
  user chat <usernameOrUUID> [numMessages] [platform] [channel]
      View chat history for a user (not yet implemented).

  user note <usernameOrUUID> <note text...>
      Add or update a note for a user.

  user merge <primaryUser> <secondaryUser>
      Merges two user accounts, combining their data.

  user roles add <username> <role>
      Adds a role to a user.

  user roles remove <username> <role>
      Removes a role from a user.

  user roles list <username>
      Lists all roles for a user.

  user analysis <usernameOrUUID>
      Shows detailed analytics for a user including message stats,
      command usage, and activity patterns.

Examples:
  user add newuser123
  user info kittyn
  user search kitt
  user note kittyn "Regular viewer, likes cats"
  user merge kittyn kittyn_alt
  user roles add kittyn moderator
  user analysis 550e8400-e29b-41d4-a716-446655440000

Note: The 'member' command has been deprecated and merged into this command.
      All member functionality is now available through the user command.
"#;