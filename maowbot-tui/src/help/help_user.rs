/// Detailed help text for the "user" command (managing user records in DB).
pub const USER_HELP_TEXT: &str = r#"User Command:
  Manages user rows in the database.

Subcommands:
  user add <username>
      Creates a new DB user with a random UUID. The 'username' will be stored
      in the global_username field.

  user remove <usernameOrUUID>
      Deletes the user from the DB. You can pass either the global_username or the user's UUID.

  user edit <usernameOrUUID>
      Prompts to update certain fields (e.g., is_active) for that user.
      Accepts either a UUID or a username.

  user info <usernameOrUUID>
      Displays details for that user (created_at, last_seen, etc.).
      Accepts either a UUID or a username.

  user search <query>
      Finds all users whose username or UUID partially matches <query>.

  user list [p [num]]
      Lists all users in the database.
      - If 'p' is provided (e.g. `user list p 50`), lists in pages with an optional page size (default=25).
      - Press ENTER after each page to continue.

Usage Examples:
  user add MyCoolUser
  user remove MyCoolUser
  user edit MyCoolUser
  user info MyCoolUser
  user edit 11111111-2222-3333-4444-555555555555
  user info 11111111-2222-3333-4444-555555555555
  user search cat
  user list
  user list p
  user list p 50
"#;