// File: maowbot-tui/src/tui_module.rs

use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering}
};
use std::io::{BufRead, BufReader, Write};
use maowbot_core::plugins::bot_api::BotApi;

use crate::commands;

use maowbot_core::eventbus::BotEvent;
use tokio::sync::mpsc::Receiver;

// We store these as simple statics for demonstration. In a real app, consider
// using a Mutex or storing them in TuiModule fields for concurrency safety.
static mut CHAT_ENABLED: bool = false;
static mut CHAT_PLATFORM_FILTER: Option<String> = None;
static mut CHAT_ACCOUNT_FILTER: Option<String> = None;

/// Let other code set chat on/off state:
pub fn set_chat_state(on: bool, platform: Option<String>, account: Option<String>) {
    unsafe {
        CHAT_ENABLED = on;
        CHAT_PLATFORM_FILTER = platform;
        CHAT_ACCOUNT_FILTER = account;
    }
}

/// TuiModule is no longer building its own runtime:
pub struct TuiModule {
    pub bot_api: Arc<dyn BotApi>,
    shutdown_flag: Arc<AtomicBool>,
}

impl TuiModule {
    pub fn new(bot_api: Arc<dyn BotApi>) -> Self {
        Self {
            bot_api,
            shutdown_flag: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Spawns the TUI in the same tokio runtime used by main,
    /// using spawn_blocking for input reading + spawn for chat printing
    pub async fn spawn_tui_thread(&self) {
        let shutdown_flag = self.shutdown_flag.clone();
        let bot_api_for_input = self.bot_api.clone();

        // 1) Synchronous TUI input in a spawn_blocking
        tokio::spawn(async move {
            // We nest spawn_blocking so that we do all blocking I/O off
            // the main async threads.
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

                    let (quit_requested, msg) = crate::commands::dispatch(line, &bot_api_for_input);

                    if let Some(output) = msg {
                        println!("{}", output);
                    }

                    if quit_requested {
                        // Instead of block_on(bot_api.shutdown()), we can just do:
                        // bot_api_for_input.shutdown().await; // but that needs an async scope
                        // or we can set a shutdown flag to signal the rest of the app:
                        shutdown_flag.store(true, Ordering::SeqCst);
                        break;
                    }
                }

                println!("(TUI) Exiting TUI thread. Goodbye!");
            }).await;
        });

        // 2) Chat printing loop in a normal tokio::spawn
        let bot_api_for_chat = self.bot_api.clone();
        tokio::spawn(async move {
            // subscribe to all chat events
            let mut rx = bot_api_for_chat.subscribe_chat_events(Some(10000)).await;
            while let Some(event) = rx.recv().await {
                if let BotEvent::ChatMessage { platform, channel, user, text, .. } = event {
                    let (enabled, pfilt, afilt) = unsafe {
                        (CHAT_ENABLED, CHAT_PLATFORM_FILTER.clone(), CHAT_ACCOUNT_FILTER.clone())
                    };
                    if !enabled {
                        continue;
                    }
                    if let Some(ref p) = pfilt {
                        if !platform.eq_ignore_ascii_case(p) {
                            continue;
                        }
                    }
                    if let Some(ref a) = afilt {
                        if !channel.eq_ignore_ascii_case(a) {
                            continue;
                        }
                    }
                    println!("{} {} {} {}", platform, channel, user, text);
                }
            }
        });
    }

    pub fn stop_tui(&self) {
        self.shutdown_flag.store(true, Ordering::SeqCst);
    }
}