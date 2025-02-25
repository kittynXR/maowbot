/// Detailed help text for the "member" command group:
///
///   - member info <usernameOrUUID>
///   - member chat <usernameOrUUID> [numMessages] [platform] [channel] [p <pageNum>] [s <search>]
///   - member list [p <pageSize>]
///   - member search <query>
///   - member note <usernameOrUUID> <note text>
///   - member merge <uuid1> <uuid2> [g <newName>]
///
/// This file is referenced by the help system (help::mod.rs) so that
/// typing `help member` will display the usage details below.

pub const MEMBER_HELP_TEXT: &str = r#"Member Command:
  Provides management and insights into bot members (also called "users" in some contexts),
  focusing on chat logs, merges, and specialized data that extends beyond simple user info.

Subcommands:
  member info <usernameOrUUID>
      Shows detailed information about that member:
       • user_id, global_username, creation & last-seen timestamps,
       • platform identities (VRChat, Twitch, etc.),
       • user analysis metrics if available.

  member chat <usernameOrUUID> [numMessages] [platform] [channel] [p <pageNum>] [s <search>]
      Displays chat messages for this member.
       • <usernameOrUUID> is required.
       • If [numMessages] is given, only that many messages are retrieved (default = all).
       • [platform], [channel], [p <pageNum>] (pagination), and [s <search>] are optional filters.
       • Example: member chat kittyn 10 twitch #coolchannel p 2 s "hello"

  member list [p <pageSize>]
      Lists all members in the database. If [p <pageSize>] is provided, the output is paginated
      with an optional page size (default=25).

  member search <query>
      Searches for members by partial match on name or user_id.

  member note <usernameOrUUID> <note text...>
      Appends or updates a moderator note on that member’s record in the user_analysis field.

  member merge <uuid1> <uuid2> [g <newGlobalUsername>]
      Merges all data from user2 (uuid2) into user1 (uuid1) and removes user2 from the system.
      Optionally sets a new global username for user1.

Examples:
  member info 81c55dd5-7212-4d0e-ab52-1d8baf952afa
  member chat kittyn 20 twitch #someChan
  member chat kittyn p 2
  member list
  member list p 50
  member search cat
  member note kittyn This user is a friend from VRChat
  member merge 22222222-3333-4444-5555-666666666666 77777777-8888-9999-0000-aaaaaaaaaaaa
"#;