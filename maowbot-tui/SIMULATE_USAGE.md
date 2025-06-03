# Using the Simulate Command

The `simulate` command allows you to trigger Twitch events without being live or waiting for actual events. This is perfect for testing bot functionality.

## Prerequisites

1. Bot must be running and connected to Twitch
2. Bot must have joined the target channel
3. You need to know the bot account name

## Basic Usage

### Send a Chat Message
```
simulate chat bot #mychannel "Hello, this is a test!"
```

### Trigger Commands
```
simulate command bot #mychannel ping
simulate command bot #mychannel followage
simulate command bot #mychannel so @someuser
```

### Simulate Redeems
```
simulate redeem bot #mychannel "Be Cute"
simulate redeem bot #mychannel "TTS" "Please read this message"
```

### Run Test Scenarios
```
simulate scenario bot #mychannel spam      # Send 5 messages quickly
simulate scenario bot #mychannel commands  # Test various commands
simulate scenario bot #mychannel mixed     # Mix of messages and commands
```

## Example Testing Session

1. First, ensure your bot is connected:
   ```
   status
   ```

2. Join a channel if needed:
   ```
   ttv join #mychannel
   ```

3. Test basic functionality:
   ```
   simulate command bot #mychannel ping
   ```

4. Test command with arguments:
   ```
   simulate command bot #mychannel so @testuser
   ```

5. Run a full scenario:
   ```
   simulate scenario bot #mychannel mixed
   ```

## Notes

- All messages appear in the actual Twitch chat
- Commands are processed by the bot's real command handlers
- The bot responds as it would to real events
- Great for testing without needing viewers or channel points