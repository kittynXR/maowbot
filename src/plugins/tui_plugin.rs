// src/plugins/tui_plugin.rs

use std::sync::Arc;
use tokio::sync::Mutex;
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
use crate::plugins::proto::plugs::{
    PluginCapability, PluginStreamResponse,
    plugin_stream_response::Payload as RespPayload,
    StatusResponse,
};

#[derive(Clone)]
pub struct TuiPlugin {
    info: Arc<Mutex<PluginConnectionInfo>>,
    plugin_manager: Arc<PluginManager>,
    event_bus: Arc<EventBus>,
    tui_task: Arc<Mutex<Option<JoinHandle<()>>>>,
}

impl TuiPlugin {
    /// Create and spawn the TUI plugin in-process
    pub fn new(
        plugin_manager: Arc<PluginManager>,
        event_bus: Arc<EventBus>,
    ) -> Self {
        let initial_info = PluginConnectionInfo {
            name: "LocalTUI".to_string(),
            capabilities: Vec::new(), // now it's Vec<PluginCapability>
        };

        let me = Self {
            info: Arc::new(Mutex::new(initial_info)),
            plugin_manager,
            event_bus,
            tui_task: Arc::new(Mutex::new(None)),
        };

        // Spawn our async TUI loop in the background
        let me_clone = me.clone();
        tokio::spawn(async move {
            me_clone.run_loop().await;
        });

        me
    }

    /// The main TUI input loop
    async fn run_loop(&self) {
        let mut guard = self.tui_task.lock().await;
        let pm = self.plugin_manager.clone();
        let eb = self.event_bus.clone();

        let handle = tokio::spawn(async move {
            let mut lines = BufReader::new(stdin()).lines();
            println!("Local TUI started. Type 'help' for commands.");

            loop {
                print!("tui> ");
                let _ = tokio::io::stdout().flush().await;
                let line_opt = lines.next_line().await;
                let line = match line_opt {
                    Ok(Some(l)) => l.trim().to_string(),
                    _ => {
                        println!("EOF or error => TUI exiting.");
                        break;
                    }
                };

                if line.is_empty() {
                    continue;
                }

                match line.as_str() {
                    "help" => {
                        println!("Commands: help, list, status, quit");
                    }
                    "list" => {
                        // "list" the plugin names from the manager
                        let names = pm.plugin_list().await;
                        println!("Connected plugins: {:?}", names);
                    }
                    "status" => {
                        // manager build_status_response -> PluginStreamResponse
                        let resp = pm.build_status_response().await;
                        // check if the payload is a StatusResponse
                        if let Some(RespPayload::StatusResponse(StatusResponse {
                                                                    connected_plugins,
                                                                    server_uptime,
                                                                })) = resp.payload
                        {
                            println!("Uptime={}s, Connected={:?}", server_uptime, connected_plugins);
                        } else {
                            println!("Unexpected response: {:?}", resp);
                        }
                    }
                    "quit" => {
                        println!("User requested bot shutdown...");
                        eb.shutdown();
                        break;
                    }
                    other => {
                        println!("Unknown command '{}'", other);
                    }
                }
            }

            println!("TUI loop ended.");
        });

        *guard = Some(handle);
    }
}

// Implement the new PluginConnection trait
#[async_trait]
impl PluginConnection for TuiPlugin {
    /// Return the plugin's info (name + capabilities).
    async fn info(&self) -> PluginConnectionInfo {
        let guard = self.info.lock().await;
        guard.clone()
    }

    /// Set the plugin's capabilities (Vec<plugs::PluginCapability>).
    async fn set_capabilities(&self, caps: Vec<PluginCapability>) {
        let mut guard = self.info.lock().await;
        guard.capabilities = caps;
    }

    /// Receive a PluginStreamResponse from the manager. We'll just print it.
    async fn send(&self, response: PluginStreamResponse) -> Result<(), Error> {
        println!("(TUI Plugin) received from manager => {:?}", response.payload);
        Ok(())
    }

    /// Called if the manager wants to stop this connection
    async fn stop(&self) -> Result<(), Error> {
        println!("(TUI Plugin) stop() called, ignoring for now.");
        Ok(())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
