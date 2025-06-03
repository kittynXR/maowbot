# TUI Test Harness

A comprehensive testing framework for the MaowBot TUI that allows testing gRPC interactions, command processing, and platform-specific functionality without requiring a running server.

## Features

- **Mock gRPC Client**: Simulate server responses without a real gRPC connection
- **Test Context**: Manage test state including users, commands, redeems, and chat messages
- **Test Fixtures**: Pre-configured test data for common scenarios
- **Test Runner**: Execute tests with colored output and detailed assertions
- **Assertion Helpers**: Fluent API for building test assertions

## Usage

From the TUI command line:

```
test_harness run-all     # Run all test suites
test_harness twitch      # Run Twitch-specific tests
test_harness commands    # Run command processing tests
test_harness redeems     # Run redeem processing tests
test_harness grpc        # Run gRPC mock tests
```

## Architecture

### MockGrpcClient
- Intercepts gRPC calls and returns pre-configured responses
- Logs all calls for verification
- Supports both static responses and dynamic callbacks

### TestContext
- Maintains test state (users, commands, redeems, messages)
- Simulates chat messages and redeem executions
- Provides assertion helpers for verifying behavior

### Test Fixtures
- Pre-built users with different roles (viewer, mod, VIP, subscriber)
- Common commands (ping, followage, vanish, shoutout)
- Channel point redeems (cute, TTS, OSC toggles)
- gRPC response templates

### TestRunner
- Manages test execution with timeouts
- Provides colored output for easy result reading
- Supports test descriptions and assertion details

## Writing Tests

```rust
TestRunner::new()
    .add_test_with_description(
        "Test Name",
        "Test description",
        |ctx| async move {
            // Setup
            ctx.add_command(fixtures::ping_command()).await;
            
            // Execute
            ctx.simulate_chat_message("user", "!ping").await;
            
            // Assert
            assert()
                .assert_true("Command was executed", 
                    ctx.assert_command_executed("ping", "user").await)
                .build()
        }
    )
```

## Test Categories

1. **Twitch Tests**: Channel info, stream status, chat settings
2. **Command Tests**: Permissions, cooldowns, argument parsing
3. **Redeem Tests**: Basic redeems, input handling, OSC toggles
4. **gRPC Tests**: Error handling, call logging, response mocking