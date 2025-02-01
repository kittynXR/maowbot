// src/plugins/tui_plugin.rs

use std::sync::Arc;
use std::io::{BufRead, BufReader};
use std::thread;
use tokio::sync::{Mutex, mpsc};
use tokio::task::JoinHandle;
use tokio::io::AsyncWriteExt;
use tracing::{info, error};
use async_trait::async_trait;
use std::any::Any;

use crate::Error;
use crate::eventbus::EventBus;
use crate::plugins::manager::{
    PluginConnection, PluginConnectionInfo, PluginManager,
};
use maowbot_proto::plugs::{
    PluginCapability, PluginStreamResponse,
    plugin_stream_response::Payload as RespPayload,
    StatusResponse,
};

#[derive(Clone)]
pub struct TuiPlugin {
    info: Arc<Mutex<PluginConnectionInfo>>,
    plugin_manager: Arc<PluginManager>,
    event_bus: Arc<EventBus>,
    _tui_task: Arc<Mutex<Option<JoinHandle<()>>>>,
}

impl TuiPlugin {
    /// Create and spawn the TUI plugin in-process.
    ///
    /// Instead of using Tokio’s async stdin (which on Windows won’t cancel when Ctrl‑C is pressed),
    /// we spawn a blocking thread that reads from stdin and sends lines over an unbounded channel.
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
            event_bus: event_bus.clone(),
            _tui_task: Arc::new(Mutex::new(None)),
        };

        // Create an unbounded channel to receive lines from a blocking thread.
        let (tx, rx) = mpsc::unbounded_channel::<String>();

        // Spawn a blocking thread to read from stdin synchronously.
        thread::spawn(move || {
            let stdin = std::io::stdin();
            let reader = BufReader::new(stdin);
            for line in reader.lines() {
                match line {
                    Ok(l) => {
                        if tx.send(l).is_err() {
                            break; // channel closed, exit thread.
                        }
                    }
                    Err(e) => {
                        eprintln!("Error reading stdin: {:?}", e);
                        break;
                    }
                }
            }
        });

        // Spawn the async TUI loop that reads from our channel.
        let me_clone = me.clone();
        tokio::spawn(async move {
            me_clone.run_loop(rx).await;
        });

        me
    }

    /// The main TUI input loop.
    ///
    /// It selects between receiving new input (sent from the blocking thread)
    /// and a shutdown signal from the event bus.
    async fn run_loop(&self, mut rx: mpsc::UnboundedReceiver<String>) {
        let eb = self.event_bus.clone();
        let pm = self.plugin_manager.clone();
        let mut shutdown_rx = eb.shutdown_rx.clone();

        println!("Local TUI started. Type 'help' for commands.");

        loop {
            // Print the prompt and flush.
            print!("tui> ");
            let _ = tokio::io::stdout().flush().await;

            tokio::select! {
                // Branch for new input from the blocking thread.
                maybe_line = rx.recv() => {
                    let line = match maybe_line {
                        Some(l) => l.trim().to_string(),
                        None => {
                            println!("Input channel closed. Exiting TUI loop.");
                            break;
                        }
                    };

                    if line.is_empty() {
                        continue;
                    }

                    match line.as_str() {
                        "help" => {
                            println!("Commands: help, list, status, quit");
                        },
                        "list" => {
                            let names = pm.plugin_list().await;
                            println!("Connected plugins: {:?}", names);
                        },
                        "status" => {
                            let resp = pm.build_status_response().await;
                            if let Some(RespPayload::StatusResponse(StatusResponse {
                                connected_plugins,
                                server_uptime,
                            })) = resp.payload {
                                println!("Uptime={}s, Connected={:?}", server_uptime, connected_plugins);
                            } else {
                                println!("Unexpected response: {:?}", resp);
                            }
                        },
                        "quit" => {
                            println!("User requested bot shutdown...");
                            eb.shutdown();
                            break;
                        },
                        other => {
                            println!("Unknown command '{}'", other);
                        }
                    }
                },

                // Branch to detect shutdown.
                Ok(_) = shutdown_rx.changed() => {
                    if *shutdown_rx.borrow() {
                        println!("Shutdown signaled. Exiting TUI loop.");
                        break;
                    }
                }
            }
        }

        println!("TUI loop ended.");
    }
}

#[async_trait]
impl PluginConnection for TuiPlugin {
    /// Return the plugin’s info (name and capabilities).
    async fn info(&self) -> PluginConnectionInfo {
        let guard = self.info.lock().await;
        guard.clone()
    }

    /// Set the plugin’s capabilities.
    async fn set_capabilities(&self, caps: Vec<PluginCapability>) {
        let mut guard = self.info.lock().await;
        guard.capabilities = caps;
    }

    async fn set_name(&self, new_name: String) {
        let mut guard = self.info.lock().await;
        guard.name = new_name;
    }

    /// Receive a PluginStreamResponse from the manager (here we simply print it).
    async fn send(&self, response: PluginStreamResponse) -> Result<(), Error> {
        println!("(TUI Plugin) received from manager => {:?}", response.payload);
        Ok(())
    }

    /// Called if the manager wants to stop this connection.
    async fn stop(&self) -> Result<(), Error> {
        println!("(TUI Plugin) stop() called, ignoring for now.");
        Ok(())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
