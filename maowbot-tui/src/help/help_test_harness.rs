pub fn help_test_harness() -> String {
    r#"
=== Test Harness Commands ===

The test harness provides a comprehensive testing framework for the TUI,
allowing you to test gRPC interactions, command processing, and more.

Usage: test_harness <subcommand>

Subcommands:
  run-all    - Run all test suites
  twitch     - Run Twitch-specific tests
  commands   - Run command processing tests  
  redeems    - Run redeem processing tests
  grpc       - Run gRPC mock tests

Examples:
  test_harness run-all     # Run all tests
  test_harness twitch      # Test Twitch functionality
  test_harness commands    # Test command handling

The test harness includes:
- Mock gRPC client for simulating server responses
- Test context for managing test state
- Fixtures for common test scenarios
- Assertion helpers for verifying behavior
- Colored output for easy test result reading

Test Categories:
1. Twitch Tests: Channel info, stream status, chat settings
2. Command Tests: Permissions, cooldowns, argument parsing
3. Redeem Tests: Basic redeems, input handling, OSC toggles
4. gRPC Tests: Error handling, call logging, response mocking

Each test provides detailed assertion output showing:
- Expected vs actual values
- Descriptive failure messages
- Execution time for each test
"#
    .to_string()
}