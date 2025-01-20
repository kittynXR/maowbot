// File: src/plugins/manager.rs

use tokio::net::{TcpListener, TcpStream};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use std::sync::{Arc, Mutex};
use crate::Error; // Or your local error type
use super::protocol::{BotToPlugin, PluginToBot};
use serde_json;
use tracing::{info, error};

/// A struct to keep track of all active plugin connections.
#[derive(Clone)]
pub struct PluginManager {
    // We store a list of connected plugins, each with a sender
    // that can write BotToPlugin messages to that plugin.
    plugins: Arc<Mutex<Vec<tokio::sync::mpsc::UnboundedSender<BotToPlugin>>>>,
}

impl PluginManager {
    pub fn new() -> Self {
        PluginManager {
            plugins: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Starts a TCP listener that accepts incoming plugin connections
    /// on the given address, e.g. "0.0.0.0:9999".
    pub async fn listen(&self, addr: &str) -> Result<(), Error> {
        let listener = TcpListener::bind(addr).await
            .map_err(|e| Error::Platform(format!("Failed to bind: {}", e)))?;
        info!("PluginManager listening on {}", addr);

        loop {
            match listener.accept().await {
                Ok((stream, _)) => {
                    let manager = self.clone();
                    tokio::spawn(async move {
                        if let Err(e) = manager.handle_connection(stream).await {
                            error!("Plugin connection error: {:?}", e);
                        }
                    });
                }
                Err(e) => {
                    error!("Failed to accept plugin connection: {:?}", e);
                }
            }
        }
    }

    /// Handle a single plugin connection.
    async fn handle_connection(&self, stream: TcpStream) -> Result<(), Error> {
        let (reader, mut writer) = stream.into_split();

        // Create a channel so we can send BotToPlugin events to this plugin.
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<BotToPlugin>();

        // Add this sender to the manager's list
        {
            let mut plugins = self.plugins.lock().unwrap();
            plugins.push(tx.clone());
        }

        // Immediately send a welcome event
        let welcome = BotToPlugin::Welcome {
            bot_name: "MaowBot".to_string(),
        };
        let msg = serde_json::to_string(&welcome).unwrap() + "\n";
        writer.write_all(msg.as_bytes()).await.unwrap();

        //--------------------------------------------------------
        // Task 1: read incoming lines from the plugin -> bot
        //--------------------------------------------------------
        let manager_read = self.clone();
        tokio::spawn(async move {
            let mut lines = BufReader::new(reader).lines();

            // lines.next_line().await? => Result<Option<String>, io::Error>
            while let Ok(Some(line)) = lines.next_line().await {
                match serde_json::from_str::<PluginToBot>(&line) {
                    Ok(parsed) => {
                        manager_read.on_plugin_message(parsed).await;
                    }
                    Err(e) => {
                        error!("Invalid JSON from plugin: {} -- line: {}", e, line);
                    }
                }
            }
        });

        //--------------------------------------------------------
        // Task 2: write outgoing events (BotToPlugin) to this plugin
        //--------------------------------------------------------
        tokio::spawn(async move {
            while let Some(evt) = rx.recv().await {
                let out = serde_json::to_string(&evt)
                    .unwrap_or_else(|_| "{\"error\":\"serialize\"}".to_string());
                if let Err(e) = writer.write_all((out + "\n").as_bytes()).await {
                    error!("Error writing to plugin: {:?}", e);
                    break;
                }
            }
        });

        Ok(())
    }

    /// Called whenever a plugin sends a `PluginToBot` message to the bot.
    async fn on_plugin_message(&self, message: PluginToBot) {
        match message {
            PluginToBot::LogMessage { text } => {
                info!("[PLUGIN LOG] {}", text);
            }
            PluginToBot::SendChat { channel, text } => {
                info!("(PLUGIN REQUEST) SendChat to {}: {}", channel, text);
                // Here you'd call your normal chat-sending logic
            }
            PluginToBot::Hello { plugin_name } => {
                info!("Plugin says hello! Plugin name: {}", plugin_name);
            }
            PluginToBot::Shutdown => {
                info!("Plugin requests shutdown. (Not yet implemented.)");
            }
        }
    }

    /// Broadcast an event to ALL connected plugins, e.g., on new chat messages.
    pub fn broadcast(&self, event: BotToPlugin) {
        let plugins = self.plugins.lock().unwrap();
        for tx in plugins.iter() {
            // Because BotToPlugin now derives Clone, we can clone the event
            let _ = tx.send(event.clone());
        }
    }
}
