use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    Mutex
};
use std::io::{stdout, Write};

use tokio::{io::BufReader};
use tokio::io::AsyncBufReadExt;

// Imported from parent module when needed
use maowbot_common_ui::GrpcClient;

/// Tracks state specific to Twitch-IRC in the TUI
#[derive(Debug)]
pub struct TtvState {
    pub active_account: Option<String>,
    pub broadcaster_channel: Option<String>,
    pub secondary_account: Option<String>,
    pub joined_channels: Vec<String>,
    pub is_in_chat_mode: bool,
    pub current_channel_index: usize,
}

impl TtvState {
    pub fn new() -> Self {
        Self {
            active_account: None,
            broadcaster_channel: None,
            secondary_account: None,
            joined_channels: Vec::new(),
            is_in_chat_mode: false,
            current_channel_index: 0,
        }
    }
}

/// Chat state for toggling chat feed on/off
#[derive(Debug, Default)]
pub struct ChatState {
    pub enabled: bool,
    pub platform_filter: Option<String>,
    pub account_filter: Option<String>,
}

/// Track OSC chat mode state
#[derive(Debug)]
pub struct OscState {
    pub is_in_chat_mode: bool,
}

impl OscState {
    pub fn new() -> Self {
        Self {
            is_in_chat_mode: false,
        }
    }
}

/// Simplified TUI module for gRPC client - no BotApi dependency
pub struct SimpleTuiModule {
    shutdown_flag: Arc<AtomicBool>,
    pub chat_state: Arc<Mutex<ChatState>>,
    pub ttv_state: Arc<Mutex<TtvState>>,
    pub osc_state: Arc<Mutex<OscState>>,
}

impl SimpleTuiModule {
    pub fn new() -> Self {
        Self {
            shutdown_flag: Arc::new(AtomicBool::new(false)),
            chat_state: Arc::new(Mutex::new(ChatState::default())),
            ttv_state: Arc::new(Mutex::new(TtvState::new())),
            osc_state: Arc::new(Mutex::new(OscState::new())),
        }
    }


    /// Called when TTV chat mode is enabled, returns `true` if the line was consumed.
    pub async fn handle_ttv_chat_line(&self, line: &str, client: &GrpcClient) -> bool {
        if line.eq_ignore_ascii_case("/quit") {
            let mut st = self.ttv_state.lock().unwrap();
            st.is_in_chat_mode = false;
            println!("Exited TTV chat mode.");
            return true;
        }
        if line.eq_ignore_ascii_case("/c") {
            let mut st = self.ttv_state.lock().unwrap();
            if !st.joined_channels.is_empty() {
                st.current_channel_index =
                    (st.current_channel_index + 1) % st.joined_channels.len();
                let new_chan = &st.joined_channels[st.current_channel_index];
                println!("Switched to channel: {}", new_chan);
            }
            return true;
        }

        // Otherwise, treat as a chat message
        let (maybe_acct, channel) = {
            let st = self.ttv_state.lock().unwrap();
            let acct = st.active_account.clone();
            let chan = if st.joined_channels.is_empty() {
                "#unknown".to_string()
            } else {
                st.joined_channels[st.current_channel_index].clone()
            };
            (acct, chan)
        };

        if let Some(account) = maybe_acct {
            // Use gRPC to send the message
            match maowbot_common_ui::commands::twitch::TwitchCommands::send_message(
                client,
                &account,
                &channel,
                line
            ).await {
                Ok(_) => {},
                Err(e) => eprintln!("Error sending chat => {:?}", e),
            }
        } else {
            eprintln!("No active Twitch-IRC account is set. Cannot send chat.");
        }
        true
    }

    /// Called when OSC chat mode is enabled, returns `true` if the line was consumed.
    pub async fn handle_osc_chat_line(&self, line: &str, client: &GrpcClient) -> bool {
        if line.eq_ignore_ascii_case("/quit") {
            let mut st = self.osc_state.lock().unwrap();
            st.is_in_chat_mode = false;
            println!("Exited OSC chatbox mode.");
            return true;
        }

        // Send the typed text to VRChat chatbox using gRPC
        match maowbot_common_ui::commands::osc::OscCommands::send_chatbox(client, line).await {
            Ok(_) => {},
            Err(e) => eprintln!("Error sending OSC chat => {:?}", e),
        }
        true
    }

    pub fn prompt_string(&self) -> String {
        // TTV chat mode has precedence in this example.
        let st_ttv = self.ttv_state.lock().unwrap();
        if st_ttv.is_in_chat_mode {
            if st_ttv.joined_channels.is_empty() {
                return "#??? > ".to_string();
            } else {
                let ch = &st_ttv.joined_channels[st_ttv.current_channel_index];
                return format!("{}> ", ch);
            }
        }

        let st_osc = self.osc_state.lock().unwrap();
        if st_osc.is_in_chat_mode {
            // just label it "chatbox"
            return "chatbox> ".to_string();
        }

        // default TUI prompt
        "tui> ".to_string()
    }

    pub async fn set_chat_state(
        &self,
        enabled: bool,
        platform: Option<String>,
        account: Option<String>,
    ) {
        let mut st = self.chat_state.lock().unwrap();
        st.enabled = enabled;
        st.platform_filter = platform;
        st.account_filter = account;
    }

    pub fn stop_tui(&self) {
        self.shutdown_flag.store(true, Ordering::SeqCst);
    }
}