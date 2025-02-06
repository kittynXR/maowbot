// File: plugins/tui_plugin/src/lib.rs

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex as StdMutex,
};
use std::io::{BufRead, BufReader, Write};
use std::thread;
use std::any::Any;
use std::str::FromStr;
use std::time::SystemTime;

use async_trait::async_trait;
use tokio::sync::Mutex;
use open; // "open" crate to spawn browser
use maowbot_core::{Error, models::Platform};
use maowbot_core::plugins::manager::{
    PluginConnection, PluginConnectionInfo,
};
use maowbot_proto::plugs::{
    PluginStreamResponse,
    plugin_stream_response::Payload as RespPayload,
    PluginCapability,
};

use maowbot_core::plugins::bot_api::{BotApi, StatusData};
use maowbot_core::auth::{AuthManager, AuthenticationPrompt, AuthenticationResponse};

/// A dynamic TUI plugin that calls back into the bot manager via `BotApi`.
#[derive(Clone)]
pub struct TuiPlugin {
    /// Basic info about this plugin (name, capabilities, etc.).
    info: Arc<StdMutex<PluginConnectionInfo>>,
    /// A flag used to shut down the TUI thread.
    shutdown_flag: Arc<AtomicBool>,
    /// Reference to the BotApi, which can be used to call `list_plugins`, `status`, etc.
    bot_api: Arc<StdMutex<Option<Arc<dyn BotApi>>>>,

    /// An optional AuthManager if the user wants to do local OAuth flows, etc.
    pub auth_manager: Option<Arc<Mutex<AuthManager>>>,
}

impl TuiPlugin {
    /// Constructor: spawns a blocking thread for the TUI.
    pub fn new() -> Self {
        let initial_info = PluginConnectionInfo {
            name: "LocalTUI".to_string(),
            capabilities: Vec::new(),
            is_enabled: true,
        };
        let me = Self {
            info: Arc::new(StdMutex::new(initial_info)),
            shutdown_flag: Arc::new(AtomicBool::new(false)),
            bot_api: Arc::new(StdMutex::new(None)),
            auth_manager: None,
        };
        me.spawn_tui_thread();
        me
    }

    /// If we want to enable OAuth flows locally, we can inject an AuthManager here.
    pub fn set_auth_manager(&mut self, auth: Arc<Mutex<AuthManager>>) {
        self.auth_manager = Some(auth);
    }

    /// Spawns the blocking TUI thread that reads commands from stdin.
    fn spawn_tui_thread(&self) {
        let shutdown_flag = self.shutdown_flag.clone();
        let bot_api_arc = self.bot_api.clone();

        // We clone self for usage inside the thread
        let plugin_clone = self.clone();

        thread::spawn(move || {
            println!("Local TUI started. Type 'help' for commands.");

            let stdin = std::io::stdin();
            let mut reader = BufReader::new(stdin);

            loop {
                print!("tui> ");
                let _ = std::io::stdout().flush();

                let mut line = String::new();
                if reader.read_line(&mut line).is_err() {
                    eprintln!("Error reading stdin");
                    break;
                }
                let trimmed = line.trim();

                // Check shutdown flag.
                if shutdown_flag.load(Ordering::SeqCst) {
                    println!("Shutdown flag => TUI thread exiting.");
                    break;
                }

                if trimmed.is_empty() {
                    continue;
                }

                // Retrieve the current BotApi, if any
                let bot_api_opt = {
                    let guard = bot_api_arc.lock().unwrap();
                    guard.clone()
                };

                let parts: Vec<&str> = trimmed.split_whitespace().collect();
                let cmd = parts[0];
                let args = &parts[1..];

                match cmd {
                    "help" => {
                        println!("Commands:");
                        println!("  help      - show this help");
                        println!("  list      - list all known plugins (with enable/disable status) and connected plugins");
                        println!("  status    - show bot status (uptime, connected plugins)");
                        println!("  plug      - usage: plug <enable|disable|remove> <pluginName>");
                        println!("  auth      - usage: auth <add|remove|list> [platform]");
                        println!("  quit      - request the bot to shut down");
                    }
                    "list" => {
                        if let Some(api) = bot_api_opt.as_ref() {
                            let all = api.list_plugins();
                            println!("All known plugins (enable/disable status):");
                            for p in all {
                                println!("  - {}", p);
                            }
                            let st = api.status();
                            println!("Currently connected plugins:");
                            for c in st.connected_plugins {
                                println!("  - {}", c);
                            }
                        } else {
                            println!("(TUI) Bot API not set => cannot list plugins");
                        }
                    }
                    "status" => {
                        if let Some(api) = bot_api_opt.as_ref() {
                            let st: StatusData = api.status();
                            println!("(TUI) Status => Uptime={}s, Connected Plugins:",
                                     st.uptime_seconds);
                            for c in st.connected_plugins {
                                println!("    {}", c);
                            }
                        } else {
                            println!("(TUI) Bot API not set => cannot get status");
                        }
                    }
                    "plug" => {
                        // Usage: plug <enable|disable|remove> <pluginName>
                        if args.len() < 2 {
                            println!("Usage: plug <enable|disable|remove> <pluginName>");
                            continue;
                        }
                        let subcmd = args[0];
                        let plugin_name = args[1];

                        match subcmd {
                            "enable" | "disable" => {
                                let enable = subcmd == "enable";
                                if let Some(api) = bot_api_opt {
                                    match api.toggle_plugin(plugin_name, enable) {
                                        Ok(_) => {
                                            println!("Plugin '{}' now {}", plugin_name,
                                                     if enable { "ENABLED" } else { "DISABLED" });
                                        }
                                        Err(e) => {
                                            println!("Error toggling plugin '{}': {:?}", plugin_name, e);
                                        }
                                    }
                                } else {
                                    println!("(TUI) Bot API not set => cannot change plugin state");
                                }
                            }
                            "remove" => {
                                if let Some(api) = bot_api_opt {
                                    match api.remove_plugin(plugin_name) {
                                        Ok(_) => {
                                            println!("Plugin '{}' was removed from the manager/JSON.", plugin_name);
                                        }
                                        Err(e) => {
                                            println!("Error removing plugin '{}': {:?}", plugin_name, e);
                                        }
                                    }
                                } else {
                                    println!("(TUI) Bot API not set => cannot remove plugin");
                                }
                            }
                            _ => {
                                println!("Usage: plug <enable|disable|remove> <pluginName>");
                            }
                        }
                    }
                    "auth" => {
                        plugin_clone.handle_auth_command(args);
                    }
                    "quit" => {
                        println!("(TUI) 'quit' => signaling bot shutdown...");
                        if let Some(api) = bot_api_opt.as_ref() {
                            api.shutdown();
                        }
                        break;
                    }
                    other => {
                        println!("(TUI) Unknown command '{}'. Type 'help' for usage.", other);
                    }
                }
            }

            println!("TUI loop ended. Goodbye!");
        });
    }

    fn handle_auth_command(&self, args: &[&str]) {
        if args.is_empty() {
            println!("Usage: auth <add|remove|list> [platform]");
            return;
        }
        match args[0] {
            "add" => {
                // If user only typed "auth add", we must show possible platforms
                if args.len() < 2 {
                    // We can list them out:
                    println!("Available platforms to add: twitch, discord, vrchat, twitch-irc");
                    return;
                }
                let platform_str = args[1];
                // Try parse
                let platform = match Platform::from_str(platform_str) {
                    Ok(p) => p,
                    Err(_) => {
                        println!("Unknown platform '{}'. Possible: twitch, discord, vrchat, twitch-irc", platform_str);
                        return;
                    }
                };
                self.cmd_auth_add(platform);
            }
            "remove" => {
                if args.len() < 3 {
                    println!("Usage: auth remove <platform> <user_id>");
                    return;
                }
                let platform_str = args[1];
                let user_id = args[2].to_string();
                let platform = match Platform::from_str(platform_str) {
                    Ok(p) => p,
                    Err(_) => {
                        println!("Unknown platform '{}'. Possible: twitch, discord, vrchat, twitch-irc", platform_str);
                        return;
                    }
                };
                self.cmd_auth_remove(platform, user_id);
            }
            "list" => {
                // Not fully implemented, but we can expand:
                println!("(TUI) 'auth list' is not fully implemented. This could show each platform’s stored credentials.");
            }
            _ => {
                println!("Usage: auth <add|remove|list> [platform] [user_id]");
            }
        }
    }

    /// Steps:
    /// 1) Ask user: "Is this a bot account or broadcaster account?" => is_bot
    /// 2) Attempt to start the OAuth flow => manager.authenticate_platform_for_role(..., is_bot)
    /// 3) The AuthManager calls into TwitchAuthenticator, which returns AuthenticationPrompt::Browser { url }
    ///    or other. We see that prompt and ask user "Open browser (Y/n)?" => if yes, open.
    /// 4) We'll have a second pass for the final code from the local callback server => we feed that back as AuthenticationResponse::Code(...)
    fn cmd_auth_add(&self, platform: Platform) {
        let auth_arc_opt = self.auth_manager.clone();
        if auth_arc_opt.is_none() {
            println!("No AuthManager configured => cannot perform local OAuth flow");
            return;
        }

        // ask user => is bot or broadcaster?
        println!("Is this a bot account? Type 'y' for bot, anything else for broadcaster:");
        let mut line = String::new();
        let _ = std::io::stdin().read_line(&mut line);
        let is_bot = line.trim().eq_ignore_ascii_case("y");

        // spawn an async task for the actual flow
        let auth_arc = auth_arc_opt.unwrap();
        tokio::spawn(async move {
            let mut manager = auth_arc.lock().await;
            // 1) call manager.authenticate_platform_for_role
            let result = manager.authenticate_platform_for_role(platform.clone(), is_bot).await;
            match result {
                Ok(prompt_cred) => {
                    // If we got a credential immediately (which might happen for simpler flows),
                    // just let user know:
                    println!("(TUI) Successfully added credentials for platform={:?}", prompt_cred.platform);
                }
                Err(Error::Auth(msg)) if msg.starts_with("Prompt:") => {
                    // Some authenticators might pass back custom text. In this example,
                    // we rely on the standard AuthenticationPrompt usage. Let's parse that:
                    // Actually we handle that differently. We'll do a second loop if needed.
                    println!("(TUI) Not used in this example: {msg}");
                }
                Err(e) => {
                    // If it's "2FA required" or something else
                    println!("(TUI) Auth flow error => {:?}", e);
                }
            }
        });
    }

    fn cmd_auth_remove(&self, platform: Platform, user_id: String) {
        let auth_arc_opt = self.auth_manager.clone();
        if auth_arc_opt.is_none() {
            println!("No AuthManager => cannot remove credentials");
            return;
        }

        tokio::spawn(async move {
            let auth_arc = auth_arc_opt.unwrap();
            let mut manager = auth_arc.lock().await;
            match manager.revoke_credentials(&platform, &user_id).await {
                Ok(_) => {
                    println!("(TUI) Removed credentials for {:?} user_id={}", platform, user_id);
                }
                Err(e) => {
                    println!("(TUI) Error removing credentials => {:?}", e);
                }
            }
        });

    }
}

#[async_trait]
impl PluginConnection for TuiPlugin {
    async fn info(&self) -> PluginConnectionInfo {
        let guard = self.info.lock().unwrap();
        guard.clone()
    }

    async fn set_capabilities(&self, capabilities: Vec<PluginCapability>) {
        let mut guard = self.info.lock().unwrap();
        guard.capabilities = capabilities;
    }

    async fn set_name(&self, new_name: String) {
        let mut guard = self.info.lock().unwrap();
        guard.name = new_name;
    }

    async fn send(&self, response: PluginStreamResponse) -> Result<(), Error> {
        let guard = self.info.lock().unwrap();
        if !guard.is_enabled {
            return Ok(());
        }
        if let Some(payload) = response.payload {
            match payload {
                RespPayload::Tick(_) => {
                    println!("(TUI) Received Tick at {:?}", SystemTime::now());
                }
                RespPayload::Welcome(w) => {
                    println!("(TUI) Received Welcome => Bot: {}", w.bot_name);
                }
                RespPayload::ChatMessage(cm) => {
                    println!("(TUI) ChatMessage => [{} #{}] {}: {}",
                             cm.platform, cm.channel, cm.user, cm.text);
                }
                RespPayload::StatusResponse(s) => {
                    println!("(TUI) Status => connected={:?}, uptime={}", s.connected_plugins, s.server_uptime);
                }
                RespPayload::CapabilityResponse(c) => {
                    println!("(TUI) Capabilities => granted={:?}, denied={:?}", c.granted, c.denied);
                }
                RespPayload::AuthError(e) => {
                    println!("(TUI) AuthError => {}", e.reason);
                }
                RespPayload::ForceDisconnect(d) => {
                    println!("(TUI) ForceDisconnect => {}", d.reason);
                }
            }
        }
        Ok(())
    }

    async fn stop(&self) -> Result<(), Error> {
        println!("(TUI) stop() called: setting shutdown flag.");
        self.shutdown_flag.store(true, Ordering::SeqCst);
        Ok(())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn set_bot_api(&self, api: Arc<dyn BotApi>) {
        let mut guard = self.bot_api.lock().unwrap();
        *guard = Some(api);
    }

    async fn set_enabled(&self, enable: bool) {
        let mut guard = self.info.lock().unwrap();
        guard.is_enabled = enable;
    }
}

/// Export the `create_plugin` symbol for dynamic loading.
#[no_mangle]
pub extern "C" fn create_plugin() -> *mut dyn PluginConnection {
    let plugin = TuiPlugin::new();
    Box::into_raw(Box::new(plugin))
}