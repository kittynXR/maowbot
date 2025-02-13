use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    Mutex
};
use std::io::{BufRead, BufReader, Write};
use tokio::runtime::Handle;
use tokio::sync::mpsc::Receiver;

use maowbot_core::eventbus::{BotEvent, EventBus};
use maowbot_core::plugins::bot_api::BotApi;

/// Tracks state specific to Twitch-IRC in the TUI:
/// - active_account: which Twitch account are we using for "join", "msg", etc.
/// - default_channel: the channel to auto-join on restart
/// - joined_channels: all channels we have joined so far
/// - is_in_chat_mode: if true, we intercept typed lines as chat
/// - current_channel_index: which channel from joined_channels is the “active” for sending
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
        // Suppose "kittyn" is our default broadcaster if nothing else is known.
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
/// (Legacy from prior TUI examples; can be used for quick text filtering.)
#[derive(Debug, Default)]
pub struct ChatState {
    pub enabled: bool,
    pub platform_filter: Option<String>,
    pub account_filter: Option<String>,
}

/// The main TUI module, which spawns:
/// 1) A blocking thread to read user input
/// 2) A background task to print chat messages
pub struct TuiModule {
    pub bot_api: Arc<dyn BotApi>,
    pub event_bus: Arc<EventBus>,

    /// Local shutdown flag for the TUI input loop
    shutdown_flag: Arc<AtomicBool>,

    /// Our shared chat-state (older design)
    pub chat_state: Arc<Mutex<ChatState>>,

    /// NEW: Holds the TTV (Twitch) state for “active account”, joined channels, chat mode, etc.
    pub ttv_state: Arc<Mutex<TtvState>>,
}

impl TuiModule {
    pub fn new(bot_api: Arc<dyn BotApi>, event_bus: Arc<EventBus>) -> Self {
        // Attempt to read from bot_config the "ttv_default_channel" if present:
        let default_channel = {
            // We do a quick block_on here since new() is sync.
            let maybe_val = tokio::runtime::Handle::current().block_on(async {
                bot_api.get_bot_config_value("ttv_default_channel").await
            });
            match maybe_val {
                Ok(Some(ch)) if !ch.is_empty() => Some(ch),
                _ => None,
            }
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

    /// Set the chat state (on/off) and optional platform/account filters.
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
    ///  - A blocking thread (spawn_blocking) to read user input
    ///  - A normal async task for printing chat messages
    pub async fn spawn_tui_thread(self: &Arc<Self>) {
        let shutdown_flag = self.shutdown_flag.clone();
        let bot_api_for_input = self.bot_api.clone();
        let event_bus_for_input = self.event_bus.clone();
        let tui_module_for_input = self.clone();

        // 1) Synchronous TUI input in a spawn_blocking
        tokio::spawn(async move {
            let _ = tokio::task::spawn_blocking(move || {
                println!("Local TUI enabled. Type 'help' for commands.\n");

                let stdin = std::io::stdin();
                let mut reader = BufReader::new(stdin);

                // If we have a default channel, auto-join it:
                // (this is purely in-memory for demonstration)
                {
                    let mut st = tui_module_for_input.ttv_state.lock().unwrap();
                    if let Some(ch) = st.default_channel.clone() {
                        if !ch.is_empty() && !st.joined_channels.contains(&ch) {
                            st.joined_channels.push(ch);
                        }
                    }

                }

                loop {
                    if shutdown_flag.load(Ordering::SeqCst) {
                        println!("(TUI) Shutting down TUI input loop...");
                        break;
                    }

                    // Check if we're in chat mode
                    let (in_chat_mode, prompt) = {
                        let t = tui_module_for_input.ttv_state.lock().unwrap();
                        if t.is_in_chat_mode {
                            let c = if t.joined_channels.is_empty() {
                                "#???".to_string()
                            } else {
                                t.joined_channels[t.current_channel_index].clone()
                            };
                            (true, format!("{}> ", c))
                        } else {
                            (false, "tui> ".to_string())
                        }
                    };

                    print!("{}", prompt);
                    let _ = std::io::stdout().flush();

                    let mut line = String::new();
                    if reader.read_line(&mut line).is_err() {
                        eprintln!("Error reading from stdin.");
                        break;
                    }
                    let line = line.trim().to_string();
                    if line.is_empty() {
                        continue;
                    }

                    if in_chat_mode {
                        // If user typed "/quit", leave chat mode
                        if line.eq_ignore_ascii_case("/quit") {
                            let mut st = tui_module_for_input.ttv_state.lock().unwrap();
                            st.is_in_chat_mode = false;
                            println!("Exited chat mode.");
                            continue;
                        }
                        // If user typed "/c", cycle channels
                        if line.eq_ignore_ascii_case("/c") {
                            let mut st = tui_module_for_input.ttv_state.lock().unwrap();
                            if !st.joined_channels.is_empty() {
                                st.current_channel_index =
                                    (st.current_channel_index + 1) % st.joined_channels.len();
                                println!("Switched to channel: {}",
                                         st.joined_channels[st.current_channel_index]);
                            }
                            continue;
                        }
                        // Otherwise, treat everything as a chat message
                        let (account, channel) = {
                            let st = tui_module_for_input.ttv_state.lock().unwrap();
                            let acct = st.active_account.clone();
                            let chan = if st.joined_channels.is_empty() {
                                "#unknown".to_string()
                            } else {
                                st.joined_channels[st.current_channel_index].clone()
                            };
                            (acct, chan)
                        };
                        let send_res = Handle::current().block_on(async {
                            bot_api_for_input
                                .send_twitch_irc_message(&account, &channel, &line)
                                .await
                        });
                        if let Err(e) = send_res {
                            eprintln!("Error sending message => {:?}", e);
                        }
                        continue;
                    }

                    let (quit_requested, msg) = crate::commands::dispatch(&line, &bot_api_for_input, &tui_module_for_input);

                    if let Some(output) = msg {
                        println!("{}", output);
                    }

                    if quit_requested {
                        event_bus_for_input.shutdown();
                        break;
                    }
                }

                println!("(TUI) Exiting TUI thread. Goodbye!");
            }).await;
        });

        // 2) Chat printing loop in a normal tokio::spawn
        let bot_api_for_chat = self.bot_api.clone();
        let module_for_chat = self.clone();

        tokio::spawn(async move {
            let mut rx = bot_api_for_chat.subscribe_chat_events(Some(10000)).await;
            while let Some(event) = rx.recv().await {
                if let BotEvent::ChatMessage { platform, channel, user, text, timestamp } = event {
                    // Acquire the current chat state
                    let st = module_for_chat.chat_state.lock().unwrap();
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

                    // Additionally, we only show it if we've actually "joined" that channel
                    let ttv_state_guard = module_for_chat.ttv_state.lock().unwrap();
                    if platform.eq_ignore_ascii_case("twitch-irc") {
                        if !ttv_state_guard.joined_channels.iter().any(|c| c.eq_ignore_ascii_case(&channel)) {
                            continue;
                        }
                    }

                    // Format: "<mm:ss>:<platform>:<#channel> <chatter>: <message>"
                    let time_str = timestamp.format("%H:%M:%S").to_string();
                    println!(
                        "{}:{}:{} {}: {}",
                        time_str, platform, channel, user, text
                    );
                }
            }
        });
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
