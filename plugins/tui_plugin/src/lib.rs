use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc
};
use std::io::{BufRead, BufReader, Write};
use std::thread;
use std::any::Any;
use std::time::SystemTime;

use async_trait::async_trait;
use tokio::sync::Mutex;

use maowbot_core::Error;
use maowbot_core::plugins::manager::{
    PluginConnection,
    PluginConnectionInfo,
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
    info: Arc<Mutex<PluginConnectionInfo>>,
    shutdown_flag: Arc<AtomicBool>,
    bot_api: Arc<Mutex<Option<Arc<dyn BotApi>>>>,
}

impl TuiPlugin {
    /// Constructor: spawns a blocking thread for the TUI.
    /// We do not require a tokio reactor for this code (it’s all std).
    pub fn new() -> Self {
        let initial_info = PluginConnectionInfo {
            name: "LocalTUI".to_string(),
            capabilities: Vec::new(),
        };
        let me = Self {
            info: Arc::new(Mutex::new(initial_info)),
            shutdown_flag: Arc::new(AtomicBool::new(false)),
            bot_api: Arc::new(Mutex::new(None)),
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

                // Check shutdown from manager
                if shutdown_flag.load(Ordering::SeqCst) {
                    println!("Shutdown flag => TUI thread exiting.");
                    break;
                }

                if trimmed.is_empty() {
                    continue;
                }

                let bot_api_opt = {
                    // Even though we’re in a std thread, we can do a blocking lock
                    // on the tokio::Mutex. It’s usually best to do an async lock,
                    // but here we’re bridging std thread <-> tokio
                    let guard = bot_api_arc.blocking_lock();
                    guard.clone()
                };

                match trimmed {
                    "help" => {
                        println!("Commands: help, list, status, quit");
                    }
                    "list" => {
                        if let Some(api) = bot_api_opt.as_ref()  {
                            let plugs = api.list_plugins();
                            println!("(TUI) Connected plugins: {:?}", plugs);
                        } else {
                            println!("(TUI) Bot API not set => cannot list plugins");
                        }
                    }
                    "status" => {
                        if let Some(api) = bot_api_opt.as_ref()  {
                            let st: StatusData = api.status();
                            println!("(TUI) Status => Uptime={}s, Plugins={:?}",
                                     st.uptime_seconds, st.connected_plugins);
                        } else {
                            println!("(TUI) Bot API not set => cannot get status");
                        }
                    }
                    "quit" => {
                        println!("(TUI) 'quit' => signal bot shutdown...");
                        if let Some(api) = bot_api_opt.as_ref()  {
                            api.shutdown();
                        }
                        break;
                    }
                    other => {
                        println!("(TUI) Unknown command '{}'", other);
                    }
                }
            }

            println!("TUI loop ended. Goodbye!");
        });
    }
}

/// Implement the `PluginConnection` trait so the bot sees us as a plugin.
#[async_trait]
impl PluginConnection for TuiPlugin {
    async fn info(&self) -> PluginConnectionInfo {
        let guard = self.info.lock().await;
        guard.clone()
    }

    async fn set_capabilities(&self, capabilities: Vec<PluginCapability>) {
        let mut guard = self.info.lock().await;
        guard.capabilities = capabilities;
    }

    async fn set_name(&self, new_name: String) {
        let mut guard = self.info.lock().await;
        guard.name = new_name;
    }

    async fn send(&self, response: PluginStreamResponse) -> Result<(), Error> {
        if let Some(payload) = response.payload {
            match payload {
                RespPayload::Tick(_) => {
                    println!("(TUI) Received Tick at {:?}", SystemTime::now());
                }
                RespPayload::Welcome(w) => {
                    println!("(TUI) Received Welcome => Bot: {}", w.bot_name);
                }
                other => {
                    println!("(TUI) Received => {:?}", other);
                }
            }
        }
        Ok(())
    }

    async fn stop(&self) -> Result<(), Error> {
        println!("(TUI) manager -> stop() => set shutdown flag => TUI loop will exit");
        self.shutdown_flag.store(true, Ordering::SeqCst);
        Ok(())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    /// The new method we added to the trait. We store the BotApi for commands.
    fn set_bot_api(&self, api: Arc<dyn BotApi>) {
        let mut guard = self.bot_api.blocking_lock();
        *guard = Some(api);
    }
}

/// Export the `create_plugin` symbol for dynamic loading.
#[no_mangle]
pub extern "C" fn create_plugin() -> *mut dyn PluginConnection {
    let plugin = TuiPlugin::new();
    Box::into_raw(Box::new(plugin))
}
