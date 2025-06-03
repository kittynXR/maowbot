pub fn help_simulate() -> String {
    r#"
=== Simulate Command ===

Trigger simulated events through the bot's Twitch IRC connection.
This allows testing commands and functionality without waiting for
actual Twitch events. Requires an active Twitch connection.

Usage: simulate <type> [args...]

Types:
  chat <account> <channel> <message>
    Send a chat message to a channel
    Example: simulate chat bot #mychannel "Hello world!"

  command <account> <channel> <command> [args...]
    Trigger a command in a channel
    Examples:
      simulate command bot #mychannel ping
      simulate command bot #mychannel so @coolstreamer
      simulate command bot #mychannel vanish @lurker

  redeem <account> <channel> <redeem_name> [input]
    Simulate a channel points redeem (via test command)
    Examples:
      simulate redeem bot #mychannel "Be Cute"
      simulate redeem bot #mychannel "TTS" "Read this message!"

  scenario <account> <channel> <type>
    Run pre-built test scenarios:
    - spam: Send multiple messages quickly
    - commands: Test various commands
    - mixed: Mix of messages and commands
    Example: simulate scenario bot #mychannel mixed

Important Notes:
- <account> is the bot account name to send from
- <channel> must include the # prefix (e.g., #mychannel)
- The bot must be connected to Twitch and joined to the channel
- Commands will be processed by the bot's normal command handlers
- Messages appear in chat as if sent by the specified account
"#
    .to_string()
}