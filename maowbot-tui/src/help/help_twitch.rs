/// Detailed help text for the "twitch" command (managing Twitch IRC usage in the TUI).
pub const TWITCH_HELP_TEXT: &str = r##"Twitch Command:
  Provides Twitch IRC controls within the TUI.

Subcommands:
  twitch active <accountName>
      Switches the active Twitch account to the specified <accountName>.
      You typically have a single broadcaster account, plus any bot accounts.

  twitch join <channelName>
      Joins the specified channel (like "#somechannel") to receive chat messages in the TUI.
      If the channel does not have a "#" prefix, it’s automatically added.

  twitch part <channelName>
      Parts (leaves) the specified channel, stopping any further messages from appearing in the TUI.

  twitch msg <channelName> <text...>
      Sends a chat message to the specified channel on the active Twitch account.

  twitch chat
      Puts the TUI into "chat mode" for Twitch IRC. The prompt changes to "#channel>",
      and any typed lines (not starting with "/") will be sent as chat messages.
      Use "/quit" to exit chat mode, and "/c" to cycle joined channels.

  twitch default <channelName>
      Sets the channel that will be automatically joined on restart (stored in bot_config).

Usage Examples:
  twitch active kittyn
  twitch join coolchannel
  twitch part #coolchannel
  twitch msg #coolchannel Hello everyone!
  twitch chat
  twitch default #coolchannel
"##;
