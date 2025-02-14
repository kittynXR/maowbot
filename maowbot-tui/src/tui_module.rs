use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    Mutex
};
use std::io::{stdout, Write};

use tokio::{sync::mpsc::Receiver, io::BufReader};
use tokio::io::AsyncBufReadExt;

use maowbot_core::eventbus::{BotEvent, EventBus};
use maowbot_core::plugins::bot_api::BotApi;
use maowbot_core::models::Platform;

/// Tracks state specific to Twitch-IRC in the TUI
#[derive(Debug)]
pub struct TtvState {
    /// If no Twitch IRC credentials exist in DB, this might be empty
    pub active_account: Option<String>,
    pub default_channel: Option<String>,
    pub joined_channels: Vec<String>,
    pub is_in_chat_mode: bool,
    pub current_channel_index: usize,
}

impl TtvState {
    // UPDATED: remove the "kittyn" and do not default to any name.
    pub fn new() -> Self {
        Self {
            active_account: None,
            default_channel: None,
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

/// The main TUI module
pub struct TuiModule {
    pub bot_api: Arc<dyn BotApi>,
    pub event_bus: Arc<EventBus>,

    shutdown_flag: Arc<AtomicBool>,

    pub chat_state: Arc<Mutex<ChatState>>,
    pub ttv_state: Arc<Mutex<TtvState>>,
}

impl TuiModule {
    // NEW: Attempt to fetch a default twitch-irc user from the DB
    pub async fn new(bot_api: Arc<dyn BotApi>, event_bus: Arc<EventBus>) -> Self {
        // Try to load a default channel from bot_config
        let default_channel = match bot_api.get_bot_config_value("ttv_default_channel").await {
            Ok(Some(ch)) if !ch.is_empty() => Some(ch),
            _ => None,
        };

        // Now see if there's any credentials for "twitch-irc".
        let ttv_creds = bot_api.list_credentials(Some(Platform::TwitchIRC)).await;
        let mut ttv_state = TtvState::new();
        ttv_state.default_channel = default_channel;

        if let Ok(creds_list) = ttv_creds {
            if !creds_list.is_empty() {
                // pick the first user_name
                ttv_state.active_account = Some(creds_list[0].user_name.clone());
            }
        }

        Self {
            bot_api,
            event_bus,
            shutdown_flag: Arc::new(AtomicBool::new(false)),
            chat_state: Arc::new(Mutex::new(ChatState::default())),
            ttv_state: Arc::new(Mutex::new(ttv_state)),
        }
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

    pub async fn spawn_tui_thread(self: &Arc<Self>) {
        let module_ref_for_input = self.clone();
        tokio::spawn(async move {
            module_ref_for_input.run_input_loop().await;
        });

        let module_ref_for_chat = self.clone();
        tokio::spawn(async move {
            module_ref_for_chat.run_chat_display_loop().await;
        });
    }

    async fn run_input_loop(self: &Arc<Self>) {
        let mut reader = BufReader::new(tokio::io::stdin()).lines();
        {
            // If we have a default channel, auto-join it:
            let mut st = self.ttv_state.lock().unwrap();
            if let Some(ch) = st.default_channel.clone() {
                if !ch.is_empty() && !st.joined_channels.contains(&ch) {
                    st.joined_channels.push(ch);
                }
            }
        }

        println!("Local TUI enabled. Type 'help' for commands.\n");

        while !self.shutdown_flag.load(Ordering::SeqCst) {
            print!("{}", self.prompt_string());
            let _ = stdout().flush();

            let line_opt = reader.next_line().await;
            if line_opt.is_err() {
                eprintln!("Error reading line from stdin. Exiting TUI...");
                break;
            }
            let line = match line_opt.unwrap() {
                Some(l) => l.trim().to_string(),
                None => break, // EOF
            };
            if line.is_empty() {
                continue;
            }

            // If chat mode is on, handle chat line
            {
                let is_in_chat_mode = {
                    let st = self.ttv_state.lock().unwrap();
                    st.is_in_chat_mode
                };
                if is_in_chat_mode {
                    if self.handle_chat_mode_line(&line).await {
                        continue;
                    }
                }
            }

            let (quit_requested, output) = crate::commands::dispatch_async(&line, &self.bot_api, self).await;
            if let Some(msg) = output {
                println!("{}", msg);
            }
            if quit_requested {
                self.event_bus.shutdown();
                break;
            }
        }

        println!("(TUI) Exiting input loop. Goodbye!");
    }

    async fn handle_chat_mode_line(&self, line: &str) -> bool {
        if line.eq_ignore_ascii_case("/quit") {
            let mut st = self.ttv_state.lock().unwrap();
            st.is_in_chat_mode = false;
            println!("Exited chat mode.");
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

        // If no account is set, cannot send chat
        if maybe_acct.is_none() {
            eprintln!("No active Twitch-IRC account is set. Cannot send chat.");
            return true;
        }

        let account = maybe_acct.unwrap();
        let res = self.bot_api.send_twitch_irc_message(&account, &channel, line).await;
        if let Err(e) = res {
            eprintln!("Error sending chat => {:?}", e);
        }
        true
    }

    fn prompt_string(&self) -> String {
        let st = self.ttv_state.lock().unwrap();
        if st.is_in_chat_mode {
            if st.joined_channels.is_empty() {
                "#??? > ".to_string()
            } else {
                let ch = &st.joined_channels[st.current_channel_index];
                format!("{}> ", ch)
            }
        } else {
            "tui> ".to_string()
        }
    }

    async fn run_chat_display_loop(&self) {
        let mut rx = self.bot_api.subscribe_chat_events(Some(10_000)).await;
        while let Some(event) = rx.recv().await {
            if let BotEvent::ChatMessage {
                platform,
                channel,
                user,
                text,
                timestamp
            } = event
            {
                let st = self.chat_state.lock().unwrap();
                if !st.enabled {
                    continue;
                }
                if let Some(ref pf) = st.platform_filter {
                    if !platform.eq_ignore_ascii_case(pf) {
                        continue;
                    }
                }
                if let Some(ref af) = st.account_filter {
                    if !channel.eq_ignore_ascii_case(af) {
                        continue;
                    }
                }

                // If it's twitch-irc, check if we've joined that channel
                if platform.eq_ignore_ascii_case("twitch-irc") {
                    let ttv_guard = self.ttv_state.lock().unwrap();
                    let joined = ttv_guard.joined_channels.iter()
                        .any(|c| c.eq_ignore_ascii_case(&channel));
                    if !joined {
                        continue;
                    }
                }

                let time_str = timestamp.format("%H:%M:%S").to_string();
                println!("{}:{}:{} {}: {}", time_str, platform, channel, user, text);
            }
        }
    }

    pub fn stop_tui(&self) {
        self.shutdown_flag.store(true, Ordering::SeqCst);
    }
}

impl std::clone::Clone for TuiModule {
    fn clone(&self) -> Self {
        Self {
            bot_api: self.bot_api.clone(),
            event_bus: self.event_bus.clone(),
            shutdown_flag: self.shutdown_flag.clone(),
            chat_state: self.chat_state.clone(),
            ttv_state: self.ttv_state.clone(),
        }
    }
}
