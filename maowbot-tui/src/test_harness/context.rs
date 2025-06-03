use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use maowbot_common::models::{user::User, command::Command, redeem::Redeem};
use crate::test_harness::mock_grpc::MockGrpcClient;
use uuid::Uuid;

#[derive(Clone)]
pub struct TestContext {
    pub grpc_client: Arc<Mutex<MockGrpcClient>>,
    pub current_platform: String,
    pub current_user: Option<User>,
    pub state: Arc<Mutex<TestState>>,
}

#[derive(Default)]
pub struct TestState {
    pub users: HashMap<Uuid, User>,
    pub commands: HashMap<String, Command>,
    pub redeems: HashMap<String, Redeem>,
    pub chat_messages: Vec<ChatMessage>,
    pub executed_commands: Vec<ExecutedCommand>,
    pub executed_redeems: Vec<ExecutedRedeem>,
}

#[derive(Clone, Debug)]
pub struct ChatMessage {
    pub platform: String,
    pub user: String,
    pub message: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

#[derive(Clone, Debug)]
pub struct ExecutedCommand {
    pub command: String,
    pub args: Vec<String>,
    pub user: String,
    pub platform: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

#[derive(Clone, Debug)]
pub struct ExecutedRedeem {
    pub redeem: String,
    pub user: String,
    pub input: Option<String>,
    pub platform: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl TestContext {
    pub fn new() -> Self {
        Self {
            grpc_client: Arc::new(Mutex::new(MockGrpcClient::new())),
            current_platform: "twitch".to_string(),
            current_user: None,
            state: Arc::new(Mutex::new(TestState::default())),
        }
    }

    pub fn with_platform(mut self, platform: String) -> Self {
        self.current_platform = platform;
        self
    }

    pub fn with_user(mut self, user: User) -> Self {
        self.current_user = Some(user);
        self
    }

    pub async fn add_user(&self, user: User) {
        let mut state = self.state.lock().await;
        state.users.insert(user.user_id, user);
    }

    pub async fn add_command(&self, command: Command) {
        let mut state = self.state.lock().await;
        state.commands.insert(command.command_name.clone(), command);
    }

    pub async fn add_redeem(&self, redeem: Redeem) {
        let mut state = self.state.lock().await;
        state.redeems.insert(redeem.reward_name.clone(), redeem);
    }

    pub async fn simulate_chat_message(&self, user: &str, message: &str) {
        let mut state = self.state.lock().await;
        state.chat_messages.push(ChatMessage {
            platform: self.current_platform.clone(),
            user: user.to_string(),
            message: message.to_string(),
            timestamp: chrono::Utc::now(),
        });

        // Check if this is a command
        if message.starts_with('!') {
            let parts: Vec<&str> = message[1..].split_whitespace().collect();
            if let Some(cmd_name) = parts.get(0) {
                if state.commands.contains_key(*cmd_name) {
                    state.executed_commands.push(ExecutedCommand {
                        command: cmd_name.to_string(),
                        args: parts[1..].iter().map(|s| s.to_string()).collect(),
                        user: user.to_string(),
                        platform: self.current_platform.clone(),
                        timestamp: chrono::Utc::now(),
                    });
                }
            }
        }
    }

    pub async fn simulate_redeem(&self, user: &str, redeem_name: &str, input: Option<String>) {
        let mut state = self.state.lock().await;
        state.executed_redeems.push(ExecutedRedeem {
            redeem: redeem_name.to_string(),
            user: user.to_string(),
            input,
            platform: self.current_platform.clone(),
            timestamp: chrono::Utc::now(),
        });
    }

    pub async fn get_chat_messages(&self) -> Vec<ChatMessage> {
        let state = self.state.lock().await;
        state.chat_messages.clone()
    }

    pub async fn get_executed_commands(&self) -> Vec<ExecutedCommand> {
        let state = self.state.lock().await;
        state.executed_commands.clone()
    }

    pub async fn get_executed_redeems(&self) -> Vec<ExecutedRedeem> {
        let state = self.state.lock().await;
        state.executed_redeems.clone()
    }

    pub async fn clear_history(&self) {
        let mut state = self.state.lock().await;
        state.chat_messages.clear();
        state.executed_commands.clear();
        state.executed_redeems.clear();
    }

    pub async fn assert_command_executed(&self, command: &str, user: &str) -> bool {
        let state = self.state.lock().await;
        state.executed_commands.iter().any(|cmd| {
            cmd.command == command && cmd.user == user
        })
    }

    pub async fn assert_redeem_executed(&self, redeem: &str, user: &str) -> bool {
        let state = self.state.lock().await;
        state.executed_redeems.iter().any(|r| {
            r.redeem == redeem && r.user == user
        })
    }

    pub async fn assert_message_sent(&self, pattern: &str) -> bool {
        let state = self.state.lock().await;
        state.chat_messages.iter().any(|msg| {
            msg.message.contains(pattern)
        })
    }
}