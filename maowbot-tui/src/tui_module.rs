use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering}
};
use std::io::{BufRead, BufReader, Write};
use tokio::sync::{mpsc::Receiver, Mutex};

use maowbot_core::eventbus::{BotEvent, EventBus};
use maowbot_core::plugins::bot_api::BotApi;


/// Holds current chat on/off state and optional filters.
/// Stored in a Mutex inside TuiModule so we can safely mutate it.
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

    /// Our shared chat-state
    pub chat_state: Arc<Mutex<ChatState>>,
}

impl TuiModule {
    pub fn new(bot_api: Arc<dyn BotApi>, event_bus: Arc<EventBus>) -> Self {
        Self {
            bot_api,
            event_bus,
            shutdown_flag: Arc::new(AtomicBool::new(false)),
            chat_state: Arc::new(Mutex::new(ChatState::default())),
        }
    }

    /// Set the chat state (on/off) and optional platform/account filters.
    /// Called by the “chat on/off” command in connectivity.rs
    pub async fn set_chat_state(
        &self,
        enabled: bool,
        platform: Option<String>,
        account: Option<String>,
    ) {
        let mut st = self.chat_state.lock().await;
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

                loop {
                    if shutdown_flag.load(Ordering::SeqCst) {
                        println!("(TUI) Shutting down TUI input loop...");
                        break;
                    }

                    print!("tui> ");
                    let _ = std::io::stdout().flush();

                    let mut line = String::new();
                    if reader.read_line(&mut line).is_err() {
                        eprintln!("Error reading from stdin.");
                        break;
                    }

                    let line = line.trim();
                    if line.is_empty() {
                        continue;
                    }

                    // Instead of the old dispatch(...) that only had bot_api,
                    // we now pass &tui_module_for_input so commands can set chat state
                    let (quit_requested, msg) =
                        crate::commands::dispatch(line, &bot_api_for_input, &tui_module_for_input);

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
                if let BotEvent::ChatMessage { platform, channel, user, text, .. } = event {
                    // Acquire the current chat state
                    let st = module_for_chat.chat_state.lock().await;
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

                    println!("[CHAT] platform={} channel={} user={} => {}",
                             platform, channel, user, text);
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
        }
    }
}
