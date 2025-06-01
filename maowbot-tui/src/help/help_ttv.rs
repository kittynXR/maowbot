/// Detailed help text for the "ttv" command (managing Twitch IRC usage in the TUI).
pub const TTV_HELP_TEXT: &str = r##"TTV Command:
  Provides Twitch IRC controls within the TUI.

Subcommands:
  ttv active <accountName>
      Switches the active Twitch account to the specified <accountName>.
      You typically have a single broadcaster account, plus any bot accounts.

  ttv join <channelName>
      Joins the specified channel (like "#somechannel") to receive chat messages in the TUI.
      If the channel does not have a "#" prefix, it’s automatically added.

  ttv part <channelName>
      Parts (leaves) the specified channel, stopping any further messages from appearing in the TUI.

  ttv msg <channelName> <text...>
      Sends a chat message to the specified channel on the active Twitch account.

  ttv chat
      Puts the TUI into "chat mode" for Twitch IRC. The prompt changes to "#channel>",
      and any typed lines (not starting with "/") will be sent as chat messages.
      Use "/quit" to exit chat mode, and "/c" to cycle joined channels.

  ttv default <channelName>
      Sets the channel that will be automatically joined on restart (stored in bot_config).

Usage Examples:
  ttv active kittyn
  ttv join coolchannel
  ttv part #coolchannel
  ttv msg #coolchannel Hello everyone!
  ttv chat
  ttv default #coolchannel
"##;
