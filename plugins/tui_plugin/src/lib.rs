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
use open; // "open" crate to spawn the user's browser
use maowbot_core::{Error, models::{Platform, PlatformCredential}};
use maowbot_core::plugins::manager::{PluginConnection, PluginConnectionInfo};
use maowbot_proto::plugs::{
    PluginStreamResponse,
    plugin_stream_response::Payload as RespPayload,
    PluginCapability,
};
use maowbot_core::plugins::bot_api::{BotApi, StatusData};

#[derive(Clone)]
pub struct TuiPlugin {
    info: Arc<StdMutex<PluginConnectionInfo>>,
    shutdown_flag: Arc<AtomicBool>,
    bot_api: Arc<StdMutex<Option<Arc<dyn BotApi>>>>,
}

impl TuiPlugin {
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
        };
        me.spawn_tui_thread();
        me
    }

    fn spawn_tui_thread(&self) {
        let shutdown_flag = self.shutdown_flag.clone();
        let bot_api_arc = self.bot_api.clone();
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

                if shutdown_flag.load(Ordering::SeqCst) {
                    println!("Shutdown flag => TUI thread exiting.");
                    break;
                }

                if trimmed.is_empty() {
                    continue;
                }

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
                        println!("  list      - list all known plugins (enable/disable status)");
                        println!("  status    - show bot status (uptime, connected plugins)");
                        println!("  plug      - usage: plug <enable|disable|remove> <pluginName>");
                        println!("  auth      - usage: auth <add|remove|list> [platform]");
                        println!("  quit      - request the bot to shut down");
                    }
                    "list" => {
                        if let Some(api) = bot_api_opt.as_ref() {
                            let all = tokio::runtime::Builder::new_current_thread()
                                .enable_all()
                                .build()
                                .unwrap()
                                .block_on(api.list_plugins());
                            println!("All known plugins:");
                            for p in all {
                                println!("  - {}", p);
                            }
                        } else {
                            println!("(TUI) Bot API not set => cannot list plugins");
                        }
                    }
                    "status" => {
                        if let Some(api) = bot_api_opt.as_ref() {
                            let rt = tokio::runtime::Builder::new_current_thread()
                                .enable_all()
                                .build()
                                .unwrap();
                            let st: StatusData = rt.block_on(api.status());

                            println!("(TUI) Uptime={}s, Connected Plugins:", st.uptime_seconds);
                            for c in st.connected_plugins {
                                println!("   {}", c);
                            }
                        } else {
                            println!("(TUI) Bot API not set => cannot get status");
                        }
                    }
                    "plug" => {
                        plugin_clone.cmd_plug(args, bot_api_opt.as_ref());
                    }
                    "auth" => {
                        plugin_clone.cmd_auth(args, bot_api_opt.as_ref());
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

    fn cmd_plug(&self, args: &[&str], api_opt: Option<&Arc<dyn BotApi>>) {
        if args.len() < 2 {
            println!("Usage: plug <enable|disable|remove> <pluginName>");
            return;
        }
        let subcmd = args[0];
        let plugin_name = args[1];

        match subcmd {
            "enable" | "disable" => {
                let enable = subcmd == "enable";
                if let Some(api) = api_opt {
                    let result = tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                        .unwrap()
                        .block_on(api.toggle_plugin(plugin_name, enable));
                    match result {
                        Ok(_) => println!("Plugin '{}' is now {}", plugin_name, if enable { "ENABLED" } else { "DISABLED" }),
                        Err(e) => println!("Error toggling plugin: {:?}", e),
                    }
                }
            }
            "remove" => {
                if let Some(api) = api_opt {
                    let result = tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                        .unwrap()
                        .block_on(api.remove_plugin(plugin_name));
                    match result {
                        Ok(_) => println!("Plugin '{}' removed.", plugin_name),
                        Err(e) => println!("Error removing '{}': {:?}", plugin_name, e),
                    }
                }
            }
            _ => {
                println!("Usage: plug <enable|disable|remove> <pluginName>");
            }
        }
    }

    fn cmd_auth(&self, args: &[&str], api_opt: Option<&Arc<dyn BotApi>>) {
        if args.is_empty() {
            println!("Usage: auth <add|remove|list> [platform]");
            return;
        }
        match args[0] {
            "add" => {
                if args.len() < 2 {
                    println!("Available platforms: twitch, discord, vrchat, twitch-irc, etc.");
                    return;
                }
                let platform_str = args[1];
                let platform = match Platform::from_str(platform_str) {
                    Ok(p) => p,
                    Err(_) => {
                        println!("Unknown platform '{}'.", platform_str);
                        return;
                    }
                };
                self.cmd_auth_add(platform, api_opt);
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
                        println!("Unknown platform '{}'.", platform_str);
                        return;
                    }
                };
                self.cmd_auth_remove(platform, &user_id, api_opt);
            }
            "list" => {
                if let Some(api) = api_opt {
                    let maybe_platform = if args.len() > 1 {
                        Platform::from_str(args[1]).ok()
                    } else {
                        None
                    };
                    let credentials = tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                        .unwrap()
                        .block_on(api.list_credentials(maybe_platform));
                    match credentials {
                        Ok(creds) => {
                            println!("Stored credentials:");
                            for c in creds {
                                println!(" - user_id={} platform={:?} is_bot={}", c.user_id, c.platform, c.is_bot);
                            }
                        }
                        Err(e) => {
                            println!("Error listing credentials => {:?}", e);
                        }
                    }
                }
            }
            _ => {
                println!("Usage: auth <add|remove|list> [platform] [user_id]");
            }
        }
    }

    fn cmd_auth_add(&self, platform: Platform, api_opt: Option<&Arc<dyn BotApi>>) {
        if api_opt.is_none() {
            println!("No BotApi => cannot add credentials");
            return;
        }
        let api = api_opt.unwrap().clone();

        println!("Is this a bot account? (y/n):");
        let mut line = String::new();
        let _ = std::io::stdin().read_line(&mut line);
        let is_bot = line.trim().eq_ignore_ascii_case("y");

        // We'll do an async block_on to call the new flow:
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        rt.block_on(async move {
            match api.begin_auth_flow(platform.clone(), is_bot).await {
                Ok(url) => {
                    println!("Open this URL to authenticate:\n  {}", url);
                    println!("Open in browser now? (y/n):");
                    let mut line = String::new();
                    let _ = std::io::stdin().read_line(&mut line);
                    if line.trim().eq_ignore_ascii_case("y") {
                        if let Err(e) = open::that(&url) {
                            println!("Could not open browser automatically: {:?}", e);
                        }
                    }
                    println!("Once you complete the OAuth in your browser, the local callback server should say 'Authentication Successful'.");
                    println!("Paste the 'code' param here (or press Enter if the code is auto-stored):");
                    let mut code_line = String::new();
                    let _ = std::io::stdin().read_line(&mut code_line);
                    let code_str = code_line.trim().to_string();
                    if code_str.is_empty() {
                        println!("No code entered => might fail if your flow isn’t auto-detected. Attempting anyway...");
                    }

                    match api.complete_auth_flow(platform.clone(), code_str).await {
                        Ok(cred) => {
                            println!("Success: Stored credentials for platform={:?}, is_bot={}", cred.platform, cred.is_bot);
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
        });
    }

    fn cmd_auth_remove(&self, platform: Platform, user_id: &str, api_opt: Option<&Arc<dyn BotApi>>) {
        if let Some(api) = api_opt {
            let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
            rt.block_on(async move {
                match api.revoke_credentials(platform.clone(), user_id).await {
                    Ok(_) => println!("Removed credentials for {:?} user_id={}", platform, user_id),
                    Err(e) => println!("Error removing credentials => {:?}", e),
                }
            });
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
        println!("(TUI) stop() called => setting shutdown flag.");
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

/// Required “create_plugin” for dynamic loading
#[no_mangle]
pub extern "C" fn create_plugin() -> *mut dyn PluginConnection {
    let plugin = TuiPlugin::new();
    Box::into_raw(Box::new(plugin))
}