// maowbot-tui/src/tui_module.rs

use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering}
};
use std::io::{BufRead, BufReader, Write};
use std::thread;
use maowbot_core::plugins::bot_api::BotApi;

use crate::commands;

/// Our “main” TUI struct
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

    /// Spawns the TUI in a background thread that listens to user input.
    pub fn spawn_tui_thread(&self) {
        let bot_api = self.bot_api.clone();
        let shutdown_flag = self.shutdown_flag.clone();

        thread::spawn(move || {
            println!("Local TUI enabled. Type 'help' for commands.\n");

            let stdin = std::io::stdin();
            let mut reader = BufReader::new(stdin);

            loop {
                // If we've been signaled to shut down externally, bail out
                if shutdown_flag.load(Ordering::SeqCst) {
                    println!("(TUI) Shutting down TUI thread...");
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

                // Dispatch command
                let (quit_requested, msg) =
                    commands::dispatch(line, &bot_api);

                if let Some(output) = msg {
                    println!("{}", output);
                }

                if quit_requested {
                    // “quit” was requested => stop the entire bot
                    bot_api.shutdown();
                    // Also set our TUI’s own shutdown flag so we exit
                    shutdown_flag.store(true, Ordering::SeqCst);
                    break;
                }
            }

            println!("(TUI) Exiting TUI thread. Goodbye!");
        });
    }

    /// If something else (like the bot’s main thread) wants to kill the TUI:
    pub fn stop_tui(&self) {
        self.shutdown_flag.store(true, Ordering::SeqCst);
    }
}