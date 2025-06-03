use std::sync::Arc;
use chrono::Utc;
use uuid::Uuid;
use serde_json::json;
use maowbot_common::traits::api::{BotApi, TwitchApi};

pub struct EventTrigger {
    bot_api: Arc<dyn BotApi>,
}

impl EventTrigger {
    pub fn new(bot_api: Arc<dyn BotApi>) -> Self {
        Self { bot_api }
    }

    // Trigger a chat message directly through IRC
    pub async fn trigger_chat_message(
        &self,
        account_name: &str,
        channel: &str,
        message: &str,
    ) -> Result<(), String> {
        // Use the Twitch IRC API to send a message
        self.bot_api.send_twitch_irc_message(account_name, channel, message)
            .await
            .map_err(|e| format!("Failed to send IRC message: {:?}", e))
    }

    // Trigger a command by sending it as a chat message
    pub async fn trigger_command(
        &self,
        account_name: &str,
        channel: &str,
        command: &str,
        args: &[&str],
    ) -> Result<(), String> {
        let message = if args.is_empty() {
            format!("!{}", command)
        } else {
            format!("!{} {}", command, args.join(" "))
        };
        
        self.trigger_chat_message(account_name, channel, &message).await
    }

    // Simulate channel points redeem via test command
    pub async fn trigger_test_redeem(
        &self,
        account_name: &str,
        channel: &str,
        redeem_name: &str,
        user_input: Option<&str>,
    ) -> Result<(), String> {
        // Send a special test command that simulates a redeem
        let message = if let Some(input) = user_input {
            format!("!test_redeem {} {}", redeem_name, input)
        } else {
            format!("!test_redeem {}", redeem_name)
        };
        
        self.trigger_chat_message(account_name, channel, &message).await
    }

    // Test scenarios
    pub async fn run_spam_test(
        &self,
        account_name: &str,
        channel: &str,
        count: usize,
    ) -> Result<(), String> {
        for i in 0..count {
            self.trigger_chat_message(
                account_name,
                channel,
                &format!("Spam test message {}", i)
            ).await?;
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }
        Ok(())
    }

    pub async fn run_command_test(
        &self,
        account_name: &str,
        channel: &str,
    ) -> Result<(), String> {
        // Test various commands
        let commands = vec![
            ("ping", vec![]),
            ("followage", vec![]),
            ("so", vec!["@testuser"]),
        ];

        for (cmd, args) in commands {
            self.trigger_command(account_name, channel, cmd, &args.iter().map(|s| *s).collect::<Vec<_>>()).await?;
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        }
        Ok(())
    }
}