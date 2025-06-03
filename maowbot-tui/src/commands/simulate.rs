use std::sync::Arc;
use maowbot_common::traits::api::BotApi;
use crate::test_harness::event_trigger::EventTrigger;
use crate::tui_module::TuiModule;

pub async fn handle_simulate_command(
    args: &[&str],
    bot_api: &Arc<dyn BotApi>,
    _tui_module: &Arc<TuiModule>,
) -> String {
    if args.is_empty() {
        return help_text();
    }

    let trigger = EventTrigger::new(bot_api.clone());
    let subcommand = args[0];

    match subcommand {
        "chat" => simulate_chat(&args[1..], &trigger).await,
        "command" => simulate_command(&args[1..], &trigger).await,
        "redeem" => simulate_redeem(&args[1..], &trigger).await,
        "scenario" => simulate_scenario(&args[1..], &trigger).await,
        _ => format!("Unknown simulation type: {}\n\n{}", subcommand, help_text()),
    }
}

async fn simulate_chat(args: &[&str], trigger: &EventTrigger) -> String {
    if args.len() < 3 {
        return "Usage: simulate chat <account> <channel> <message>".to_string();
    }

    let account = args[0];
    let channel = args[1];
    let message = args[2];

    match trigger.trigger_chat_message(account, channel, message).await {
        Ok(_) => format!("✓ Sent message to {} as {}: {}", channel, account, message),
        Err(e) => format!("✗ Failed to send message: {}", e),
    }
}

async fn simulate_command(args: &[&str], trigger: &EventTrigger) -> String {
    if args.len() < 3 {
        return "Usage: simulate command <account> <channel> <command> [args...]".to_string();
    }

    let account = args[0];
    let channel = args[1];
    let command = args[2];
    let cmd_args: Vec<&str> = if args.len() > 3 { args[3..].to_vec() } else { vec![] };

    match trigger.trigger_command(account, channel, command, &cmd_args).await {
        Ok(_) => format!("✓ Triggered !{} in {} as {}", command, channel, account),
        Err(e) => format!("✗ Failed: {}", e),
    }
}

async fn simulate_redeem(args: &[&str], trigger: &EventTrigger) -> String {
    if args.len() < 3 {
        return "Usage: simulate redeem <account> <channel> <redeem_name> [input]".to_string();
    }

    let account = args[0];
    let channel = args[1];
    let redeem_name = args[2];
    let input = args.get(3).map(|s| *s);

    match trigger.trigger_test_redeem(account, channel, redeem_name, input).await {
        Ok(_) => format!("✓ Triggered test redeem '{}' in {} as {}", redeem_name, channel, account),
        Err(e) => format!("✗ Failed: {}", e),
    }
}

async fn simulate_scenario(args: &[&str], trigger: &EventTrigger) -> String {
    if args.len() < 3 {
        return "Usage: simulate scenario <account> <channel> <type>\nTypes: spam, commands, mixed".to_string();
    }

    let account = args[0];
    let channel = args[1];
    let scenario_type = args[2];

    match scenario_type {
        "spam" => {
            match trigger.run_spam_test(account, channel, 5).await {
                Ok(_) => "✓ Spam scenario completed".to_string(),
                Err(e) => format!("✗ Spam scenario failed: {}", e),
            }
        }
        "commands" => {
            match trigger.run_command_test(account, channel).await {
                Ok(_) => "✓ Command test scenario completed".to_string(),
                Err(e) => format!("✗ Command scenario failed: {}", e),
            }
        }
        "mixed" => {
            let mut results = vec![];
            
            // Send some regular messages
            for i in 0..3 {
                match trigger.trigger_chat_message(account, channel, &format!("Test message {}", i)).await {
                    Ok(_) => results.push(format!("✓ Message {}", i)),
                    Err(e) => results.push(format!("✗ Message {} failed: {}", i, e)),
                }
                tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
            }
            
            // Trigger some commands
            for cmd in &["ping", "followage"] {
                match trigger.trigger_command(account, channel, cmd, &[]).await {
                    Ok(_) => results.push(format!("✓ Command !{}", cmd)),
                    Err(e) => results.push(format!("✗ Command !{} failed: {}", cmd, e)),
                }
                tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;
            }
            
            results.join("\n")
        }
        _ => format!("Unknown scenario type: {}", scenario_type),
    }
}

fn help_text() -> String {
    r#"
=== Simulate Command ===

Trigger simulated events through the bot's Twitch IRC connection.
Requires an active Twitch connection with appropriate credentials.

Usage: simulate <type> [args...]

Types:
  chat <account> <channel> <message>
    Send a chat message to a channel
    Example: simulate chat bot #mychannel "Hello world!"

  command <account> <channel> <command> [args...]
    Trigger a command in a channel
    Example: simulate command bot #mychannel ping
    Example: simulate command bot #mychannel so @coolstreamer

  redeem <account> <channel> <redeem_name> [input]
    Simulate a channel points redeem (via test command)
    Example: simulate redeem bot #mychannel "Be Cute"
    Example: simulate redeem bot #mychannel "TTS" "Read this!"

  scenario <account> <channel> <type>
    Run pre-built test scenarios
    Types: spam, commands, mixed
    Example: simulate scenario bot #mychannel mixed

Notes:
- <account> is the bot account name to send from
- <channel> must include the # prefix (e.g., #mychannel)
- Commands will be processed by the bot's command handlers
- Make sure the bot is connected to the specified channel first
"#.to_string()
}