use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex as StdMutex,
};
use std::io::{BufRead, BufReader, Write};
use std::thread;
use std::any::Any;
use std::time::SystemTime;

use async_trait::async_trait;

// Make sure to import the tokio Mutex (not std::sync::Mutex)
use tokio::sync::Mutex;

use maowbot_core::Error;
use maowbot_core::plugins::manager::{
    PluginConnection, PluginConnectionInfo,
};
use maowbot_proto::plugs::{
    PluginStreamResponse,
    plugin_stream_response::Payload as RespPayload,
    PluginCapability,
};

use maowbot_core::plugins::bot_api::{BotApi, StatusData};

// For our new "auth" commands:
use std::str::FromStr;
use maowbot_core::models::Platform;
use maowbot_core::auth::AuthManager;

/// A dynamic TUI plugin that calls back into the bot manager via `BotApi`.
#[derive(Clone)]
pub struct TuiPlugin {
    /// Basic info about this plugin (name, capabilities, etc.).
    info: Arc<StdMutex<PluginConnectionInfo>>,
    /// A flag used to shut down the TUI thread.
    shutdown_flag: Arc<AtomicBool>,
    /// Reference to the BotApi, which can be used to call `list_plugins`, `status`, etc.
    bot_api: Arc<StdMutex<Option<Arc<dyn BotApi>>>>,

    /// Optional: an AuthManager behind a tokio async Mutex so we can call
    /// `authenticate_platform`, `revoke_credentials`, etc. from the TUI.
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
            auth_manager: None, // start with None unless we add a setter or new param
        };
        me.spawn_tui_thread();
        me
    }

    /// Allows us to set (or update) the AuthManager later if desired.
    /// Usage: `tui_plugin.set_auth_manager(Arc::new(Mutex::new(auth_mgr)))`
    pub fn set_auth_manager(&mut self, auth: Arc<Mutex<AuthManager>>) {
        self.auth_manager = Some(auth);
    }

    /// Spawns the blocking TUI thread that reads commands from stdin.
    fn spawn_tui_thread(&self) {
        let shutdown_flag = self.shutdown_flag.clone();
        let bot_api_arc = self.bot_api.clone();

        // Clone self to call instance methods (cmd_auth_add, etc.)
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
                        println!("  auth      - usage: auth <add|remove|list> [platform] [user_id?]");
                        println!("  quit      - request the bot to shut down");
                    }
                    "list" => {
                        if let Some(api) = bot_api_opt.as_ref() {
                            let all = api.list_plugins();
                            println!("All known plugins (with enable/disable status):");
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
                                // The new subcommand that removes the plugin from JSON
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
                    // Our new "auth" command
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

    /// Allows "auth" subcommands: add, remove, list
    fn handle_auth_command(&self, args: &[&str]) {
        // e.g.: auth add <platform> <user_id>
        if args.is_empty() {
            println!("Usage: auth <add|remove|list> [platform] [user_id]");
            return;
        }
        match args[0] {
            "add" => {
                if args.len() < 3 {
                    println!("Usage: auth add <platform> <user_id>");
                    return;
                }
                let platform_str = args[1].to_string();
                let user_id = args[2].to_string();
                self.cmd_auth_add(platform_str, user_id);
            }
            "remove" => {
                if args.len() < 3 {
                    println!("Usage: auth remove <platform> <user_id>");
                    return;
                }
                let platform_str = args[1].to_string();
                let user_id = args[2].to_string();
                self.cmd_auth_remove(platform_str, user_id);
            }
            "list" => {
                // optional platform
                let maybe_platform = args.get(1).map(|p| p.to_string());
                self.cmd_auth_list(maybe_platform);
            }
            _ => {
                println!("Usage: auth <add|remove|list> [platform] [user_id]");
            }
        }
    }

    /// Spawns a task that calls AuthManager::authenticate_platform
    fn cmd_auth_add(&self, platform_str: String, user_id: String) {
        // If we have an AuthManager, do an async call
        if let Some(auth_arc) = self.auth_manager.clone() {
            tokio::spawn(async move {
                let mut guard = auth_arc.lock().await;
                match Platform::from_str(&platform_str) {
                    Ok(platform) => {
                        match guard.authenticate_platform(platform).await {
                            Ok(cred) => {
                                println!("(TUI) Successfully added credentials for platform={:?}", cred.platform);
                            }
                            Err(e) => {
                                println!("(TUI) Error authenticating: {:?}", e);
                            }
                        }
                    }
                    Err(e) => {
                        println!("(TUI) Invalid platform '{}': {:?}", platform_str, e);
                    }
                }
            });
        } else {
            println!("(TUI) No AuthManager => cannot do 'auth add'");
        }
    }

    /// Spawns a task that calls AuthManager::revoke_credentials
    fn cmd_auth_remove(&self, platform_str: String, user_id: String) {
        if let Some(auth_arc) = self.auth_manager.clone() {
            tokio::spawn(async move {
                let mut guard = auth_arc.lock().await;
                match Platform::from_str(&platform_str) {
                    Ok(platform) => {
                        match guard.revoke_credentials(&platform, &user_id).await {
                            Ok(_) => {
                                println!("(TUI) Removed credentials for platform={} user={}", platform_str, user_id);
                            }
                            Err(e) => {
                                println!("(TUI) Error removing credentials: {:?}", e);
                            }
                        }
                    }
                    Err(e) => {
                        println!("(TUI) Invalid platform '{}': {:?}", platform_str, e);
                    }
                }
            });
        } else {
            println!("(TUI) No AuthManager => cannot do 'auth remove'");
        }
    }

    /// Spawns a task that calls AuthManager::get_credentials or enumerates them
    fn cmd_auth_list(&self, maybe_platform: Option<String>) {
        if let Some(auth_arc) = self.auth_manager.clone() {
            tokio::spawn(async move {
                let guard = auth_arc.lock().await;
                if let Some(plat_str) = maybe_platform {
                    match Platform::from_str(&plat_str) {
                        Ok(platform) => {
                            let cred_opt = guard.get_credentials(&platform, "someUserId").await;
                            match cred_opt {
                                Ok(Some(c)) => {
                                    println!("(TUI) Found credential => platform={:?}, user_id={}", c.platform, c.user_id);
                                }
                                Ok(None) => {
                                    println!("(TUI) No credential found for platform={}", plat_str);
                                }
                                Err(e) => println!("(TUI) Error retrieving credential => {:?}", e),
                            }
                        }
                        Err(e) => {
                            println!("(TUI) Invalid platform '{}': {:?}", plat_str, e);
                        }
                    }
                } else {
                    println!("(TUI) 'auth list' of all credentials is not implemented in this example.");
                }
            });
        } else {
            println!("(TUI) No AuthManager => cannot do 'auth list'");
        }
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

    /// The Bot API can be set after construction if desired.
    fn set_bot_api(&self, api: Arc<dyn BotApi>) {
        let mut guard = self.bot_api.lock().unwrap();
        *guard = Some(api);
    }

    async fn set_enabled(&self, enable: bool) {
        let mut guard = self.info.lock();
        guard.unwrap().is_enabled = enable;
    }
}

/// Export the `create_plugin` symbol for dynamic loading.
#[no_mangle]
pub extern "C" fn create_plugin() -> *mut dyn PluginConnection {
    let plugin = TuiPlugin::new();
    Box::into_raw(Box::new(plugin))
}