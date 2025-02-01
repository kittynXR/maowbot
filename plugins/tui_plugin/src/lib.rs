use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex as StdMutex,
};
use std::io::{BufRead, BufReader, Write};
use std::thread;
use std::any::Any;
use std::time::SystemTime;

use async_trait::async_trait;
// Use the standard (blocking) Mutex for the TUI thread.
use std::sync::Mutex;

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

/// A dynamic TUI plugin that calls back into the bot manager via `BotApi`.
#[derive(Clone)]
pub struct TuiPlugin {
    info: Arc<StdMutex<PluginConnectionInfo>>,
    shutdown_flag: Arc<AtomicBool>,
    bot_api: Arc<StdMutex<Option<Arc<dyn BotApi>>>>,
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
        };
        me.spawn_tui_thread();
        me
    }

    /// Spawns the blocking TUI thread that reads commands from stdin.
    fn spawn_tui_thread(&self) {
        let shutdown_flag = self.shutdown_flag.clone();
        let bot_api_arc = self.bot_api.clone();

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
                        println!("  plug      - usage: plug <enable|disable> <pluginName>");
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
                        // Usage: plug <enable|disable> <pluginName>
                        if args.len() < 2 {
                            println!("Usage: plug <enable|disable> <pluginName>");
                            continue;
                        }
                        let subcmd = args[0];
                        let plugin_name = args[1];
                        let enable = match subcmd {
                            "enable" => true,
                            "disable" => false,
                            _ => {
                                println!("Usage: plug <enable|disable> <pluginName>");
                                continue;
                            }
                        };
                        if let Some(api) = bot_api_opt {
                            // Now simply call the new toggle_plugin method.
                            match api.toggle_plugin(plugin_name, enable) {
                                Ok(_) => println!("Plugin '{}' now {}", plugin_name, if enable { "ENABLED" } else { "DISABLED" }),
                                Err(e) => println!("Error toggling plugin '{}': {:?}", plugin_name, e),
                            }
                        } else {
                            println!("(TUI) Bot API not set => cannot change plugin state");
                        }
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
}

/// Export the `create_plugin` symbol for dynamic loading.
#[no_mangle]
pub extern "C" fn create_plugin() -> *mut dyn PluginConnection {
    let plugin = TuiPlugin::new();
    Box::into_raw(Box::new(plugin))
}