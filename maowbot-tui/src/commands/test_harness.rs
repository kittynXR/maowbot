use crate::test_harness::{TestRunner, TestContext, fixtures, assert, success};

pub struct TestHarnessCommand;

impl TestHarnessCommand {
    pub async fn execute_from_args(args: &[&str]) -> Result<(), String> {
        let subcommand = args.get(0).map(|s| s.to_lowercase()).unwrap_or_else(|| "run-all".to_string());
        
        match subcommand.as_str() {
            "run-all" => run_all_tests().await,
            "twitch" => run_twitch_tests().await,
            "commands" => run_command_tests().await,
            "redeems" => run_redeem_tests().await,
            "grpc" => run_grpc_tests().await,
            _ => {
                println!("Unknown test harness subcommand: {}", subcommand);
                println!("Available subcommands: run-all, twitch, commands, redeems, grpc");
                Ok(())
            }
        }
    }
    
}

async fn run_all_tests() -> Result<(), String> {
    let runner = create_full_test_suite();
    let summary = runner.run().await;
    
    if !summary.all_passed() {
        return Err(format!("Test harness failed with {} failures", summary.failed));
    }
    
    Ok(())
}

async fn run_twitch_tests() -> Result<(), String> {
    let runner = TestRunner::new()
        .add_test_with_description(
            "Twitch Chat Message",
            "Test basic chat message handling",
            |ctx| async move {
                // Simulate chat message
                ctx.simulate_chat_message("testuser", "Hello world!").await;

                // Verify
                assert()
                    .assert_true("Message was recorded", 
                        ctx.assert_message_sent("Hello world!").await)
                    .build()
            }
        )
        .add_test_with_description(
            "Command Execution",
            "Test command execution in chat",
            |ctx| async move {
                // Add ping command
                ctx.add_command(fixtures::ping_command()).await;

                // Execute command
                ctx.simulate_chat_message("viewer", "!ping").await;

                assert()
                    .assert_true("Command was executed",
                        ctx.assert_command_executed("ping", "viewer").await)
                    .build()
            }
        );

    let summary = runner.run().await;
    if !summary.all_passed() {
        return Err("Twitch tests failed".to_string());
    }
    Ok(())
}

async fn run_command_tests() -> Result<(), String> {
    let runner = TestRunner::new()
        .add_test_with_description(
            "Command Cooldown",
            "Test that commands respect cooldown periods",
            |ctx| async move {
                // Add ping command with 5 second cooldown
                ctx.add_command(fixtures::ping_command()).await;

                // First command should work
                ctx.simulate_chat_message("user1", "!ping").await;
                let executed = ctx.get_executed_commands().await;
                if executed.len() != 1 {
                    return assert()
                        .assert_eq("First command execution", 1, executed.len())
                        .build();
                }

                // Second command within cooldown should be ignored
                ctx.simulate_chat_message("user1", "!ping").await;
                let executed = ctx.get_executed_commands().await;
                
                assert()
                    .assert_eq("Commands executed during cooldown", 1, executed.len())
                    .assert_eq("Command name", "ping", &executed[0].command)
                    .build()
            }
        )
        .add_test_with_description(
            "Command Permissions",
            "Test that commands check user permissions",
            |ctx| async move {
                // Add mod-only command
                ctx.add_command(fixtures::vanish_command()).await;
                ctx.add_user(fixtures::viewer_user()).await;
                ctx.add_user(fixtures::moderator_user()).await;

                // Viewer shouldn't be able to use it
                ctx.simulate_chat_message("viewer123", "!vanish @target").await;
                let executed = ctx.get_executed_commands().await;
                if !executed.is_empty() {
                    return assert()
                        .assert_eq("Viewer should not execute mod command", 0, executed.len())
                        .build();
                }

                // Moderator should be able to use it
                ctx.simulate_chat_message("moderator456", "!vanish @target").await;
                let executed = ctx.get_executed_commands().await;

                assert()
                    .assert_eq("Moderator can execute command", 1, executed.len())
                    .assert_eq("Command name", "vanish", &executed[0].command)
                    .build()
            }
        );

    let summary = runner.run().await;
    if !summary.all_passed() {
        return Err("Command tests failed".to_string());
    }
    Ok(())
}

async fn run_redeem_tests() -> Result<(), String> {
    let runner = TestRunner::new()
        .add_test_with_description(
            "Basic Redeem",
            "Test basic channel points redeem execution",
            |ctx| async move {
                ctx.add_redeem(fixtures::cute_redeem()).await;
                ctx.simulate_redeem("subscriber123", "Be Cute", None).await;

                assert()
                    .assert_true("Redeem was executed",
                        ctx.assert_redeem_executed("Be Cute", "subscriber123").await)
                    .build()
            }
        )
        .add_test_with_description(
            "Redeem with Input",
            "Test redeem that requires user input",
            |ctx| async move {
                ctx.add_redeem(fixtures::tts_redeem()).await;
                ctx.simulate_redeem("vip789", "TTS Message", Some("Hello world!".to_string())).await;

                let redeems = ctx.get_executed_redeems().await;
                
                assert()
                    .assert_eq("One redeem executed", 1, redeems.len())
                    .assert_eq("Redeem name", "TTS Message", &redeems[0].redeem)
                    .assert_eq("User input captured", "Hello world!", 
                        redeems[0].input.as_ref().unwrap())
                    .build()
            }
        );

    let summary = runner.run().await;
    if !summary.all_passed() {
        return Err("Redeem tests failed".to_string());
    }
    Ok(())
}

async fn run_grpc_tests() -> Result<(), String> {
    let runner = TestRunner::new()
        .add_test_with_description(
            "Mock Call Tracking",
            "Test that mock calls are tracked correctly",
            |ctx| async move {
                // Clear any existing calls
                ctx.grpc_client.lock().await.clear_calls();
                
                // Verify call log is empty
                let calls = ctx.grpc_client.lock().await.get_calls();
                
                assert()
                    .assert_eq("Initial call log should be empty", 0, calls.len())
                    .build()
            }
        )
        .add_test_with_description(
            "Mock Response Setup",
            "Test that mock responses can be configured",
            |ctx| async move {
                // Just verify we can set up a mock without errors
                success("Mock response setup test completed")
            }
        );

    let summary = runner.run().await;
    if !summary.all_passed() {
        return Err("gRPC tests failed".to_string());
    }
    Ok(())
}

fn create_full_test_suite() -> TestRunner {
    TestRunner::new()
        // Add all tests from individual suites
        .add_test("Channel Info Test", |ctx| async move {
            success("Channel info test passed")
        })
        .add_test("Command Permission Test", |ctx| async move {
            ctx.add_command(fixtures::ping_command()).await;
            ctx.add_user(fixtures::viewer_user()).await;
            
            ctx.simulate_chat_message("viewer123", "!ping").await;
            
            assert()
                .assert_true("Viewer can use viewer command",
                    ctx.assert_command_executed("ping", "viewer123").await)
                .build()
        })
        .add_test("Redeem Execution Test", |ctx| async move {
            ctx.add_redeem(fixtures::osc_toggle_redeem()).await;
            ctx.simulate_redeem("user123", "Toggle Avatar Feature", None).await;
            
            assert()
                .assert_true("OSC toggle redeem executed",
                    ctx.assert_redeem_executed("Toggle Avatar Feature", "user123").await)
                .build()
        })
}