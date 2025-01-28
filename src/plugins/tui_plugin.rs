use std::sync::Arc;
use tokio::sync::{Mutex, mpsc};
use tokio::task::JoinHandle;
use tokio::io::{AsyncBufReadExt, BufReader, AsyncWriteExt, stdin};
use tracing::{info, error};

use async_trait::async_trait;
use std::any::Any;

use crate::Error;
use crate::eventbus::EventBus;
use crate::plugins::manager::{
    PluginConnection,
    PluginConnectionInfo,
    PluginManager,
};
use crate::plugins::protocol::{
    BotToPlugin,
    PluginToBot,
};
use crate::plugins::capabilities::PluginCapability;

/// A text-based UI plugin that runs in-process.  We store the join handle
/// in `tui_task` so we could stop/join it if we wanted.
#[derive(Clone)]
pub struct TuiPlugin {
    info: Arc<Mutex<PluginConnectionInfo>>,
    plugin_manager: Arc<PluginManager>,
    event_bus: Arc<EventBus>,

    /// Optional handle to the background TUI reading task.
    tui_task: Arc<Mutex<Option<JoinHandle<()>>>>,
}

impl TuiPlugin {
    /// Create the TuiPlugin, then spawn the TUI loop in the background.
    ///
    /// This must be called from within a running tokio context (so `tokio::spawn` works).
    pub fn new(
        plugin_manager: Arc<PluginManager>,
        event_bus: Arc<EventBus>,
    ) -> Self {
        let initial_info = PluginConnectionInfo {
            name: "LocalTUI".to_string(),
            capabilities: Vec::new(),
        };

        let me = Self {
            info: Arc::new(Mutex::new(initial_info)),
            plugin_manager,
            event_bus,
            tui_task: Arc::new(Mutex::new(None)),
        };

        // Immediately spawn the TUI loop asynchronously.
        let me_clone = me.clone();
        tokio::spawn(async move {
            me_clone.spawn_tui_loop().await;
        });

        me
    }

    /// The actual TUI loop. We store the returned JoinHandle in `tui_task`.
    /// Note `spawn_tui_loop` is `async fn` so we can use `.lock().await`.
    pub async fn spawn_tui_loop(&self) {
        // Grab mutable access to `tui_task` so we can store the JoinHandle.
        let mut task_guard = self.tui_task.lock().await;

        // We'll clone these references into the new background task:
        let pm = self.plugin_manager.clone();
        let eb = self.event_bus.clone();
        let info_arc = self.info.clone();

        // Spawn a real async task that reads lines from stdin in a loop:
        let handle = tokio::spawn(async move {
            let mut stdin_reader = BufReader::new(stdin()).lines();
            println!("Welcome to the Local TUI! Type 'help' for commands.");

            loop {
                print!("tui> ");
                // must flush stdout in async
                let _ = tokio::io::stdout().flush().await;

                let line_opt = stdin_reader.next_line().await;
                let line = match line_opt {
                    Ok(Some(l)) => l.trim().to_string(),
                    Ok(None) => {
                        println!("EOF on stdin. TUI exiting.");
                        break;
                    }
                    Err(e) => {
                        error!("Error reading line: {:?}", e);
                        break;
                    }
                };

                if line.is_empty() {
                    continue;
                }

                let parts: Vec<&str> = line.split_whitespace().collect();
                let cmd = parts[0].to_lowercase();

                match cmd.as_str() {
                    "help" => {
                        println!("Commands:");
                        println!("  help              - Show this help");
                        println!("  list              - Show connected plugins");
                        println!("  status            - Show status (uptime, plugin list)");
                        println!("  start_listening   - Start plugin TCP listener");
                        println!("  stop_listening    - Stop plugin TCP listener");
                        println!("  quit              - Shutdown the entire bot");
                    }
                    "list" => {
                        let list = pm.plugin_list().await;
                        println!("Currently connected plugins: {:?}", list);
                    }
                    "status" => {
                        let resp = pm.build_status_response().await;
                        match resp {
                            BotToPlugin::StatusResponse { connected_plugins, server_uptime } => {
                                println!("Server uptime: {}s", server_uptime);
                                println!("Connected plugins: {:?}", connected_plugins);
                            }
                            other => println!("Unexpected response: {:?}", other),
                        }
                    }
                    "start_listening" => {
                        match pm.start_listening().await {
                            Ok(_) => println!("Listening started (or already running)."),
                            Err(e) => println!("Error starting listener: {:?}", e),
                        }
                    }
                    "stop_listening" => {
                        pm.stop_listening().await;
                        println!("Stopped listening.");
                    }
                    "quit" => {
                        println!("TUI requesting bot shutdown...");
                        eb.shutdown();
                        break;
                    }
                    _ => {
                        println!("Unknown command: '{}'. Type 'help' for usage.", cmd);
                    }
                }
            }

            println!("TUI loop finished.");
        });

        // Store the handle in `tui_task`
        *task_guard = Some(handle);
    }
}

// We still implement `PluginConnection` so it appears in PluginManager’s plugin list.
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

    async fn send(&self, event: BotToPlugin) -> Result<(), Error> {
        // For example, just print the event
        println!("(TUIPlugin) received event: {:?}", event);
        Ok(())
    }

    async fn stop(&self) -> Result<(), Error> {
        println!("(TUIPlugin) stop() called. We do nothing special here.");
        Ok(())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
