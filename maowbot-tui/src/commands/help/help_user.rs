/// Detailed help text for the "user" command (managing user records in DB).
pub const USER_HELP_TEXT: &str = r#"User Command:
  Manages user rows in the database.

Subcommands:
  user add <username>
      Creates a new DB user with a random UUID. The 'username' will be stored
      in the global_username field.

  user remove <usernameOrUUID>
      Deletes the user from DB. You can pass either the global_username or the user's UUID.

  user edit <UUID>
      Prompts to update certain fields (e.g., is_active) for that user.

  user info <UUID>
      Displays details for that user (created_at, last_seen, etc.).

  user search <query>
      Finds all users whose username or UUID partially matches <query>.

Usage Examples:
  user add MyCoolUser
  user remove MyCoolUser
  user edit 11111111-2222-3333-4444-555555555555
  user info 11111111-2222-3333-4444-555555555555
  user search cat
"#;
