use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    Mutex as StdMutex
};
use std::io::{BufRead, BufReader, Write};
use std::thread;
use std::time::SystemTime;
use std::any::Any;
use std::str::FromStr;

use open; // "open" crate to spawn the user's browser if desired.

use maowbot_core::models::Platform;
use maowbot_core::plugins::bot_api::{BotApi, StatusData};

/// A small struct to hold our TUI's internal state:
pub struct TuiModule {
    /// We hold a reference to the main Bot API so we can list plugins, toggle them, etc.
    bot_api: Arc<dyn BotApi>,
    /// Set true if the TUI thread should shut down.
    shutdown_flag: Arc<AtomicBool>,
}

impl TuiModule {
    /// Create a new TuiModule, storing the `bot_api`.
    pub fn new(bot_api: Arc<dyn BotApi>) -> Self {
        Self {
            bot_api,
            shutdown_flag: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Spawn a thread that repeatedly reads lines from stdin and executes TUI commands.
    /// This function immediately returns; the thread runs in the background.
    pub fn spawn_tui_thread(&self) {
        let shutdown_flag = self.shutdown_flag.clone();
        let bot_api = self.bot_api.clone();

        thread::spawn(move || {
            println!("Local TUI enabled. Type 'help' for commands.");

            let stdin = std::io::stdin();
            let mut reader = BufReader::new(stdin);

            loop {
                print!("tui> ");
                let _ = std::io::stdout().flush();

                let mut line = String::new();
                if reader.read_line(&mut line).is_err() {
                    eprintln!("Error reading from stdin.");
                    break;
                }
                let trimmed = line.trim();

                // Check if we need to shut down
                if shutdown_flag.load(Ordering::SeqCst) {
                    println!("TUI shutting down...");
                    break;
                }

                if trimmed.is_empty() {
                    continue;
                }

                // Split the user's input into command + arguments
                let parts: Vec<&str> = trimmed.split_whitespace().collect();
                let cmd = parts[0];
                let args = &parts[1..];

                match cmd {
                    "help" => {
                        println!("Commands:");
                        println!("  help        - show this help");
                        println!("  list        - list all known plugins (enable/disable status)");
                        println!("  status      - show bot status (uptime, connected plugins)");
                        println!("  plug        - usage: plug <enable|disable|remove> <pluginName>");
                        println!("  auth        - usage: auth <add|remove|list> [platform] [user_id]");
                        println!("  quit        - request the bot to shut down");
                    }
                    "list" => {
                        // List known plugins
                        let all = match tokio::runtime::Builder::new_current_thread()
                            .enable_all()
                            .build()
                        {
                            Ok(rt) => rt.block_on(bot_api.list_plugins()),
                            Err(e) => {
                                println!("(TUI) Could not spawn runtime: {:?}", e);
                                continue;
                            }
                        };
                        println!("All known plugins:");
                        for p in all {
                            println!("  - {}", p);
                        }
                    }
                    "status" => {
                        let status_data = match tokio::runtime::Builder::new_current_thread()
                            .enable_all()
                            .build()
                        {
                            Ok(rt) => rt.block_on(bot_api.status()),
                            Err(e) => {
                                println!("(TUI) Could not get status: {:?}", e);
                                continue;
                            }
                        };
                        println!("(TUI) Uptime={}s, Connected Plugins:", status_data.uptime_seconds);
                        for c in status_data.connected_plugins {
                            println!("   {}", c);
                        }
                    }
                    "plug" => {
                        if args.len() < 2 {
                            println!("Usage: plug <enable|disable|remove> <pluginName>");
                            continue;
                        }
                        let subcmd = args[0];
                        let plugin_name = args[1];
                        match subcmd {
                            "enable" | "disable" => {
                                let enable = subcmd == "enable";
                                let result = tokio::runtime::Builder::new_current_thread()
                                    .enable_all()
                                    .build()
                                    .unwrap()
                                    .block_on(bot_api.toggle_plugin(plugin_name, enable));
                                match result {
                                    Ok(_) => println!("Plugin '{}' is now {}", plugin_name,
                                                      if enable { "ENABLED" } else { "DISABLED" }),
                                    Err(e) => println!("Error toggling plugin: {:?}", e),
                                }
                            }
                            "remove" => {
                                let result = tokio::runtime::Builder::new_current_thread()
                                    .enable_all()
                                    .build()
                                    .unwrap()
                                    .block_on(bot_api.remove_plugin(plugin_name));
                                match result {
                                    Ok(_) => println!("Plugin '{}' removed.", plugin_name),
                                    Err(e) => println!("Error removing '{}': {:?}", plugin_name, e),
                                }
                            }
                            _ => {
                                println!("Usage: plug <enable|disable|remove> <pluginName>");
                            }
                        }
                    }
                    "auth" => {
                        // Auth flow commands
                        // auth <add|remove|list> [platform] [user_id]
                        if args.is_empty() {
                            println!("Usage: auth <add|remove|list> [platform] [user_id]");
                            continue;
                        }
                        match args[0] {
                            "add" => {
                                if args.len() < 2 {
                                    println!("Usage: auth add <platform>");
                                    println!("Examples of platforms: twitch, discord, vrchat, twitch-irc, etc.");
                                    continue;
                                }
                                let platform_str = args[1];
                                match Platform::from_str(platform_str) {
                                    Ok(platform) => {
                                        TuiModule::auth_add_flow(platform, &bot_api);
                                    }
                                    Err(_) => {
                                        println!("Unknown platform '{}'", platform_str);
                                    }
                                }
                            }
                            "remove" => {
                                if args.len() < 3 {
                                    println!("Usage: auth remove <platform> <user_id>");
                                    continue;
                                }
                                let platform_str = args[1];
                                let user_id = args[2].to_string();
                                match Platform::from_str(platform_str) {
                                    Ok(platform) => {
                                        TuiModule::auth_remove(platform, &user_id, &bot_api);
                                    }
                                    Err(_) => {
                                        println!("Unknown platform '{}'", platform_str);
                                    }
                                }
                            }
                            "list" => {
                                // Optionally filter by platform
                                let maybe_platform = if args.len() > 1 {
                                    Platform::from_str(args[1]).ok()
                                } else {
                                    None
                                };
                                let result = tokio::runtime::Builder::new_current_thread()
                                    .enable_all()
                                    .build()
                                    .unwrap()
                                    .block_on(bot_api.list_credentials(maybe_platform));
                                match result {
                                    Ok(creds) => {
                                        println!("Stored credentials:");
                                        for c in creds {
                                            println!(" - user_id={} platform={:?} is_bot={}",
                                                     c.user_id, c.platform, c.is_bot);
                                        }
                                    }
                                    Err(e) => {
                                        println!("Error listing credentials => {:?}", e);
                                    }
                                }
                            }
                            _ => {
                                println!("Usage: auth <add|remove|list> [platform] [user_id]");
                            }
                        }
                    }
                    "quit" => {
                        println!("(TUI) 'quit' => shutting down the entire bot...");
                        // Call the bot's shutdown
                        bot_api.shutdown();
                        break;
                    }
                    other => {
                        println!("(TUI) Unknown command '{}'. Type 'help' for usage.", other);
                    }
                }
            }
            println!("(TUI) Exiting TUI thread. Goodbye!");
        });
    }

    /// If the user wants to begin an OAuth flow for the given platform
    fn auth_add_flow(platform: Platform, bot_api: &Arc<dyn BotApi>) {
        println!("Is this a bot account? (y/n):");
        let mut line = String::new();
        let _ = std::io::stdin().read_line(&mut line);
        let is_bot = line.trim().eq_ignore_ascii_case("y");

        let rt = match tokio::runtime::Builder::new_current_thread().enable_all().build() {
            Ok(rt) => rt,
            Err(e) => {
                println!("Error creating runtime: {:?}", e);
                return;
            }
        };

        // Step 1: begin auth flow => get redirect URL
        match rt.block_on(bot_api.begin_auth_flow(platform.clone(), is_bot)) {
            Ok(url) => {
                println!("Open this URL to authenticate:\n  {}", url);
                println!("Open in browser now? (y/n):");
                let mut line2 = String::new();
                let _ = std::io::stdin().read_line(&mut line2);
                if line2.trim().eq_ignore_ascii_case("y") {
                    if let Err(err) = open::that(&url) {
                        println!("Could not open browser automatically: {:?}", err);
                    }
                }
                println!("After finishing OAuth in the browser, if a 'code' param was displayed");
                println!("enter it here (or just press Enter if code is auto-handled): ");
                let mut code_line = String::new();
                let _ = std::io::stdin().read_line(&mut code_line);
                let code_str = code_line.trim().to_string();
                // Step 2: complete the flow
                match rt.block_on(bot_api.complete_auth_flow(platform.clone(), code_str)) {
                    Ok(cred) => {
                        println!("Success! Stored credentials for platform={:?}, is_bot={}",
                                 cred.platform, cred.is_bot);
                    }
                    Err(e) => {
                        println!("Error completing auth => {:?}", e);
                    }
                }
            }
            Err(e) => {
                println!("Error beginning auth flow => {:?}", e);
            }
        }
    }

    /// If the user wants to remove (revoke) a particular credential.
    fn auth_remove(platform: Platform, user_id: &str, bot_api: &Arc<dyn BotApi>) {
        let rt = match tokio::runtime::Builder::new_current_thread().enable_all().build() {
            Ok(rt) => rt,
            Err(e) => {
                println!("Error creating runtime: {:?}", e);
                return;
            }
        };

        match rt.block_on(bot_api.revoke_credentials(platform.clone(), user_id)) {
            Ok(_) => {
                println!("Removed credentials for {:?} user_id={}", platform, user_id);
            }
            Err(e) => {
                println!("Error removing credentials => {:?}", e);
            }
        }
    }

    /// Set shutdown flag
    pub fn stop_tui(&self) {
        self.shutdown_flag.store(true, Ordering::SeqCst);
    }
}