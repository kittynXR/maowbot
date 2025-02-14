// maowbot-tui/src/tui_module.rs

use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    Mutex
};
use std::io::{stdout, Write};

use tokio::{sync::mpsc::Receiver, io::BufReader};
use tokio::io::{AsyncBufReadExt, Stdin};

use maowbot_core::eventbus::{BotEvent, EventBus};
use maowbot_core::plugins::bot_api::BotApi;

/// Tracks state specific to Twitch-IRC in the TUI:
/// - active_account: which Twitch account we’re using for "join", "msg", etc.
/// - default_channel: the channel to auto-join on restart
/// - joined_channels: all channels we have joined so far
/// - is_in_chat_mode: if true, we intercept typed lines as chat
/// - current_channel_index: which channel from joined_channels is “active” for sending
#[derive(Debug)]
pub struct TtvState {
    pub active_account: String,
    pub default_channel: Option<String>,
    pub joined_channels: Vec<String>,
    pub is_in_chat_mode: bool,
    pub current_channel_index: usize,
}

impl TtvState {
    pub fn new() -> Self {
        // Suppose "kittyn" is our default broadcaster if nothing else is known
        Self {
            active_account: "kittyn".to_string(),
            default_channel: None,
            joined_channels: Vec::new(),
            is_in_chat_mode: false,
            current_channel_index: 0,
        }
    }
}

/// Holds current chat on/off state and optional platform/account filters.
#[derive(Debug, Default)]
pub struct ChatState {
    pub enabled: bool,
    pub platform_filter: Option<String>,
    pub account_filter: Option<String>,
}

/// The main TUI module, which spawns:
///   (1) an asynchronous task to read user input from stdin
///   (2) an asynchronous task to print chat messages
pub struct TuiModule {
    pub bot_api: Arc<dyn BotApi>,
    pub event_bus: Arc<EventBus>,

    /// Local shutdown flag for the TUI input loop
    shutdown_flag: Arc<AtomicBool>,

    /// Our shared chat-state (older design)
    pub chat_state: Arc<Mutex<ChatState>>,

    /// Holds the Twitch (TTV) state for “active account,” joined channels, etc.
    pub ttv_state: Arc<Mutex<TtvState>>,
}

impl TuiModule {
    pub async fn new(bot_api: Arc<dyn BotApi>, event_bus: Arc<EventBus>) -> Self {
        // Attempt to load a “ttv_default_channel” from bot_config
        let default_channel = match bot_api.get_bot_config_value("ttv_default_channel").await {
            Ok(Some(ch)) if !ch.is_empty() => Some(ch),
            _ => None,
        };

        let mut initial_state = TtvState::new();
        initial_state.default_channel = default_channel;

        Self {
            bot_api,
            event_bus,
            shutdown_flag: Arc::new(AtomicBool::new(false)),
            chat_state: Arc::new(Mutex::new(ChatState::default())),
            ttv_state: Arc::new(Mutex::new(initial_state)),
        }
    }

    /// Turn chat feed on or off (and optionally filter by platform/account).
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

    /// Spawns the TUI:
    ///   (1) an asynchronous task to read user input
    ///   (2) an asynchronous task to display chat messages
    pub async fn spawn_tui_thread(self: &Arc<Self>) {
        let module_ref_for_input = self.clone();
        tokio::spawn(async move {
            module_ref_for_input.run_input_loop().await;
        });

        // Also spawn a task for printing chat messages
        let module_ref_for_chat = self.clone();
        tokio::spawn(async move {
            module_ref_for_chat.run_chat_display_loop().await;
        });
    }

    /// Asynchronous loop to read lines from stdin and dispatch commands
    async fn run_input_loop(self: &Arc<Self>) {
        let mut reader = BufReader::new(tokio::io::stdin()).lines();

        // If we have a default channel, auto-join it:
        {
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
                None => {
                    // EOF encountered, exit TUI
                    break;
                }
            };

            if line.is_empty() {
                continue;
            }

            // If we are in chat mode, handle that separately
            {
                let is_in_chat_mode = {
                    let st = self.ttv_state.lock().unwrap();
                    st.is_in_chat_mode
                };
                if is_in_chat_mode {
                    if self.handle_chat_mode_line(&line).await {
                        // If handle_chat_mode_line == true, we continue reading next line
                        continue;
                    }
                }
            }

            // Otherwise, we dispatch normal TUI commands
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

    /// When in chat mode, user lines are interpreted as chat messages, or chat-mode commands.
    /// Returns `true` if we handled it, so the caller can continue reading the next line.
    async fn handle_chat_mode_line(&self, line: &str) -> bool {
        // Special chat-mode slash commands:
        //   /quit -> leave chat mode
        //   /c    -> cycle channels
        if line.eq_ignore_ascii_case("/quit") {
            let mut st = self.ttv_state.lock().unwrap();
            st.is_in_chat_mode = false;
            println!("Exited chat mode.");
            return true;
        }
        if line.eq_ignore_ascii_case("/c") {
            let mut st = self.ttv_state.lock().unwrap();
            if !st.joined_channels.is_empty() {
                st.current_channel_index = (st.current_channel_index + 1) % st.joined_channels.len();
                let new_chan = &st.joined_channels[st.current_channel_index];
                println!("Switched to channel: {}", new_chan);
            }
            return true;
        }

        // Otherwise, treat everything as a chat message
        let (account, channel) = {
            let st = self.ttv_state.lock().unwrap();
            let acct = st.active_account.clone();
            let chan = if st.joined_channels.is_empty() {
                "#unknown".to_string()
            } else {
                st.joined_channels[st.current_channel_index].clone()
            };
            (acct, chan)
        };
        let res = self.bot_api.send_twitch_irc_message(&account, &channel, line).await;
        if let Err(e) = res {
            eprintln!("Error sending chat => {:?}", e);
        }
        true
    }

    /// A small helper to produce the correct prompt string:
    ///   - if we’re in chat mode, show `#channel> `
    ///   - otherwise show `tui> `
    fn prompt_string(&self) -> String {
        let st = self.ttv_state.lock().unwrap();
        if st.is_in_chat_mode {
            if st.joined_channels.is_empty() {
                "#??? > ".to_string()
            } else {
                format!("{}> ", st.joined_channels[st.current_channel_index])
            }
        } else {
            "tui> ".to_string()
        }
    }

    /// Asynchronous loop that prints chat messages to console if chat is enabled
    async fn run_chat_display_loop(&self) {
        let mut rx = self.bot_api.subscribe_chat_events(Some(10_000)).await;

        while let Some(event) = rx.recv().await {
            if let BotEvent::ChatMessage { platform, channel, user, text, timestamp } = event {
                // Acquire the current chat state
                let st = self.chat_state.lock().unwrap();
                if !st.enabled {
                    continue;
                }
                // optional filtering
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

                // Check if we joined that channel in TTV sense
                if platform.eq_ignore_ascii_case("twitch-irc") {
                    let ttv_guard = self.ttv_state.lock().unwrap();
                    let joined = ttv_guard.joined_channels.iter().any(|c| c.eq_ignore_ascii_case(&channel));
                    if !joined {
                        continue;
                    }
                }

                // Print out the chat message
                let time_str = timestamp.format("%H:%M:%S").to_string();
                println!("{}:{}:{} {}: {}", time_str, platform, channel, user, text);
            }
        }
    }

    /// Called if you want to programmatically stop the TUI
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