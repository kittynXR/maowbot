// File: src/plugins/manager.rs

use super::protocol::{BotToPlugin, PluginToBot};
use super::capabilities::{PluginCapability, GrantedCapabilities};
use crate::Error;
use serde_json;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, mpsc};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;
use tracing::{error, info};

use crate::eventbus::{EventBus, BotEvent};
use crate::plugins::capabilities::RequestedCapabilities;
use async_trait::async_trait;
use std::any::Any;
use tokio::task::JoinHandle;

/// Holds static info about one connected plugin (name + assigned capabilities).
#[derive(Clone)]
pub struct PluginConnectionInfo {
    pub name: String,
    pub capabilities: Vec<PluginCapability>,
}

#[async_trait]
pub trait PluginConnection: Send + Sync {
    async fn info(&self) -> PluginConnectionInfo;
    async fn set_capabilities(&self, capabilities: Vec<PluginCapability>);
    async fn send(&self, event: BotToPlugin) -> Result<(), Error>;
    async fn stop(&self) -> Result<(), Error>;
    fn as_any(&self) -> &dyn Any;
}

/// Example for a TCP-based plugin connection
pub struct TcpPluginConnection {
    info: Arc<Mutex<PluginConnectionInfo>>,
    sender: mpsc::UnboundedSender<BotToPlugin>,
}

impl TcpPluginConnection {
    pub fn new(name: String, sender: mpsc::UnboundedSender<BotToPlugin>) -> Self {
        let info = PluginConnectionInfo {
            name,
            capabilities: Vec::new(),
        };
        Self {
            info: Arc::new(Mutex::new(info)),
            sender,
        }
    }
}

#[async_trait]
impl PluginConnection for TcpPluginConnection {
    async fn info(&self) -> PluginConnectionInfo {
        let guard = self.info.lock().await;
        guard.clone()
    }

    async fn set_capabilities(&self, capabilities: Vec<PluginCapability>) {
        let mut guard = self.info.lock().await;
        guard.capabilities = capabilities;
    }

    async fn send(&self, event: BotToPlugin) -> Result<(), Error> {
        self.sender
            .send(event)
            .map_err(|_| Error::Platform("Failed to send to plugin channel".into()))
    }

    async fn stop(&self) -> Result<(), Error> {
        let _ = self
            .send(BotToPlugin::ForceDisconnect {
                reason: "Manager stopping connection".to_string(),
            })
            .await;
        Ok(())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Example of an in-process plugin
pub struct DynamicPluginConnection {
    info: Arc<Mutex<PluginConnectionInfo>>,
}

impl DynamicPluginConnection {
    pub fn load_dynamic_plugin(path: &str) -> Result<Self, Error> {
        let info = PluginConnectionInfo {
            name: format!("dynamic_plugin_from_{}", path),
            capabilities: Vec::new(),
        };
        Ok(Self {
            info: Arc::new(Mutex::new(info)),
        })
    }
}

#[async_trait]
impl PluginConnection for DynamicPluginConnection {
    async fn info(&self) -> PluginConnectionInfo {
        let guard = self.info.lock().await;
        guard.clone()
    }

    async fn set_capabilities(&self, capabilities: Vec<PluginCapability>) {
        let mut guard = self.info.lock().await;
        guard.capabilities = capabilities;
    }

    async fn send(&self, event: BotToPlugin) -> Result<(), Error> {
        info!("(InProcess) sending event: {:?}", event);
        Ok(())
    }

    async fn stop(&self) -> Result<(), Error> {
        info!("(InProcess) stopping dynamic plugin...");
        Ok(())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// The `PluginManager` holds all active plugin connections, plus the passphrase (if any).
#[derive(Clone)]
pub struct PluginManager {
    /// All current plugin connections
    pub plugins: Arc<Mutex<Vec<Arc<dyn PluginConnection>>>>,

    /// Optional passphrase for plugin authentication
    passphrase: Option<String>,

    /// Track start time for a “server uptime” statistic
    start_time: std::time::Instant,

    /// Optionally store an EventBus reference for publishing/consuming
    event_bus: Option<Arc<EventBus>>,

    /// A handle to the background listener task, if we’re currently listening.
    listen_handle: Arc<Mutex<Option<JoinHandle<()>>>>,
}

impl PluginManager {
    /// Create a new PluginManager.
    pub fn new(passphrase: Option<String>) -> Self {
        Self {
            plugins: Arc::new(Mutex::new(Vec::new())),
            passphrase,
            start_time: std::time::Instant::now(),
            event_bus: None,
            listen_handle: Arc::new(Mutex::new(None)),
        }
    }

    /// For convenience in tests, or for TUI to see what’s connected, etc.
    pub async fn plugin_list(&self) -> Vec<String> {
        let lock = self.plugins.lock().await;
        lock.iter()
            .map(|p| futures_lite::future::block_on(p.info()).name)
            .collect()
    }

    /// Assign an EventBus to the manager. Called from main/server init.
    pub fn set_event_bus(&mut self, bus: Arc<EventBus>) {
        self.event_bus = Some(bus);
    }

    /// Load an in-process plugin from a shared library or DLL.
    pub fn load_in_process_plugin(&self, path: &str) -> Result<(), Error> {
        let dynamic = DynamicPluginConnection::load_dynamic_plugin(path)?;
        let mut plugins = self.plugins.blocking_lock();
        plugins.push(Arc::new(dynamic));
        Ok(())
    }

    /// Public method to start listening on `addr` if not already.
    /// If we already have a running listener, do nothing.
    pub async fn start_listening(&self) -> Result<(), Error> {
        // If we already have a handle, do nothing
        let mut lock = self.listen_handle.lock().await;
        if lock.is_some() {
            // already listening
            return Ok(());
        }

        let addr = "127.0.0.1:9999"; // or store it if you want a param
        info!("PluginManager starting to listen on {}", addr);

        let manager = self.clone();
        let handle = tokio::spawn(async move {
            if let Err(e) = manager.listen(addr).await {
                error!("PluginManager listen error: {:?}", e);
            }
        });

        *lock = Some(handle);
        Ok(())
    }

    /// Public method to stop listening if we have a running listener.
    pub async fn stop_listening(&self) {
        let mut lock = self.listen_handle.lock().await;
        if let Some(h) = lock.take() {
            h.abort();
            info!("PluginManager: listening task aborted.");
        }
    }

    /// Start listening for plugin TCP (plaintext). This is the “core” method,
    /// but now called from `start_listening()`.
    async fn listen(&self, addr: &str) -> Result<(), Error> {
        let listener = TcpListener::bind(addr)
            .await
            .map_err(|e| Error::Platform(format!("Failed to bind: {}", e)))?;
        info!("PluginManager listening on {}", addr);

        loop {
            match listener.accept().await {
                Ok((stream, _)) => {
                    let manager = self.clone();
                    tokio::spawn(async move {
                        if let Err(e) = manager.handle_tcp_connection(stream).await {
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

    async fn handle_tcp_connection<T>(&self, stream: T) -> Result<(), Error>
    where
        T: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static,
    {
        let (reader, mut writer) = tokio::io::split(stream);
        let mut lines = BufReader::new(reader).lines();
        let (tx, mut rx) = mpsc::unbounded_channel::<BotToPlugin>();

        // Expect a "Hello" message first
        let line = match lines.next_line().await {
            Ok(Some(line)) => line,
            _ => {
                error!("No data from plugin, closing.");
                return Ok(());
            }
        };

        let hello_msg: PluginToBot = match serde_json::from_str(&line) {
            Ok(msg) => msg,
            Err(_) => {
                let err_msg = BotToPlugin::AuthError {
                    reason: "Expected Hello as first message".to_string(),
                };
                let out = serde_json::to_string(&err_msg)? + "\n";
                writer.write_all(out.as_bytes()).await?;
                error!("First message not Hello. Closing.");
                return Ok(());
            }
        };

        let plugin_name = match hello_msg {
            PluginToBot::Hello { plugin_name, passphrase } => {
                if let Some(req) = &self.passphrase {
                    if Some(req.clone()) != passphrase {
                        let err_msg = BotToPlugin::AuthError {
                            reason: "Invalid passphrase".into(),
                        };
                        let out = serde_json::to_string(&err_msg)? + "\n";
                        writer.write_all(out.as_bytes()).await?;
                        error!("Plugin '{}' invalid passphrase!", plugin_name);
                        return Ok(());
                    }
                }
                plugin_name
            }
            _ => {
                let err_msg = BotToPlugin::AuthError {
                    reason: "Expected Hello as first message".to_string(),
                };
                let out = serde_json::to_string(&err_msg)? + "\n";
                writer.write_all(out.as_bytes()).await?;
                error!("First message not Hello. Closing.");
                return Ok(());
            }
        };

        // Create a new plugin connection
        let tcp_plugin = TcpPluginConnection::new(plugin_name.clone(), tx.clone());
        let plugin_arc = Arc::new(tcp_plugin);

        // Push it into our manager's list
        {
            let mut plugins = self.plugins.lock().await;
            plugins.push(plugin_arc.clone());
        }

        // Send a Welcome
        let welcome = BotToPlugin::Welcome {
            bot_name: "MaowBot".to_string(),
        };
        let msg = serde_json::to_string(&welcome)? + "\n";
        writer.write_all(msg.as_bytes()).await?;

        // Inbound read loop
        let pm_clone = self.clone();
        let plugin_name_clone = plugin_name.clone();
        tokio::spawn(async move {
            while let Ok(Some(line)) = lines.next_line().await {
                match serde_json::from_str::<PluginToBot>(&line) {
                    Ok(msg) => {
                        pm_clone
                            .on_plugin_message(msg, &plugin_name_clone, plugin_arc.clone())
                            .await;
                    }
                    Err(e) => {
                        error!(
                            "Invalid JSON from plugin {}: {} line={}",
                            plugin_name_clone, e, line
                        );
                    }
                }
            }
            info!("Plugin '{}' read loop ended.", plugin_name_clone);

            // Remove from manager
            let mut plugins = pm_clone.plugins.lock().await;
            if let Some(idx) = plugins
                .iter()
                .position(|p| futures_lite::future::block_on(p.info()).name == plugin_name_clone)
            {
                plugins.remove(idx);
            }
        });

        // Outbound write loop
        tokio::spawn(async move {
            while let Some(evt) = rx.recv().await {
                let out = serde_json::to_string(&evt)
                    .unwrap_or_else(|_| "{\"error\":\"serialize\"}".into())
                    + "\n";
                if writer.write_all(out.as_bytes()).await.is_err() {
                    error!("Error writing to plugin. Possibly disconnected.");
                    break;
                }
            }
            info!("Plugin '{}' write loop ended.", plugin_name);
        });

        Ok(())
    }

    pub async fn on_plugin_message(
        &self,
        message: PluginToBot,
        plugin_name: &str,
        plugin_conn: Arc<dyn PluginConnection>,
    ) {
        match message {
            PluginToBot::LogMessage { text } => {
                info!("[PLUGIN LOG: {}] {}", plugin_name, text);
            }
            PluginToBot::RequestStatus => {
                let status = self.build_status_response().await;
                let _ = plugin_conn.send(status).await;
            }
            PluginToBot::Shutdown => {
                info!("Plugin '{}' requests a bot shutdown. Stopping...", plugin_name);
                if let Some(bus) = &self.event_bus {
                    bus.shutdown();
                }
            }
            PluginToBot::SwitchScene { scene_name } => {
                let info = plugin_conn.info().await;
                if info.capabilities.contains(&PluginCapability::SceneManagement) {
                    info!("Plugin '{}' requests scene switch: {}", plugin_name, scene_name);
                } else {
                    let err = BotToPlugin::AuthError {
                        reason: "No SceneManagement capability.".into(),
                    };
                    let _ = plugin_conn.send(err).await;
                }
            }
            PluginToBot::SendChat { channel, text } => {
                let info = plugin_conn.info().await;
                if info.capabilities.contains(&PluginCapability::SendChat) {
                    info!("(PLUGIN->BOT) {} says: send chat to {} -> {}", plugin_name, channel, text);

                    if let Some(bus) = &self.event_bus {
                        let evt = BotEvent::ChatMessage {
                            platform: "plugin".to_string(),
                            channel,
                            user: plugin_name.to_string(),
                            text,
                            timestamp: chrono::Utc::now(),
                        };
                        let _ = bus.publish(evt).await;
                    }
                } else {
                    let err = BotToPlugin::AuthError {
                        reason: "No SendChat capability.".into(),
                    };
                    let _ = plugin_conn.send(err).await;
                }
            }
            PluginToBot::RequestCapabilities(req) => {
                info!(
                    "Plugin '{}' requests capabilities: {:?}",
                    plugin_name, req.requested
                );
                let (granted, denied) = self.evaluate_capabilities(&req.requested);
                plugin_conn.set_capabilities(granted.clone()).await;

                let response = BotToPlugin::CapabilityResponse(GrantedCapabilities { granted, denied });
                let _ = plugin_conn.send(response).await;
            }
            PluginToBot::Hello { .. } => {
                error!("Plugin '{}' sent Hello again unexpectedly.", plugin_name);
            }
        }
    }

    fn evaluate_capabilities(
        &self,
        requested: &[PluginCapability],
    ) -> (Vec<PluginCapability>, Vec<PluginCapability>) {
        let mut granted = vec![];
        let mut denied = vec![];
        for cap in requested {
            if *cap == PluginCapability::ChatModeration {
                denied.push(cap.clone());
            } else {
                granted.push(cap.clone());
            }
        }
        (granted, denied)
    }

    /// Example method to handle ChatMessage from bus. Called by `subscribe_to_event_bus`.
    async fn handle_chat_event(&self, platform: &str, channel: &str, user: &str, text: &str) {
        info!(
            "PluginManager: handle_chat_event => {} #{} (user={}, text={})",
            platform, channel, user, text
        );
        let plugins = self.plugins.lock().await;
        for p in plugins.iter() {
            let p_info = p.info().await;
            if p_info.capabilities.contains(&PluginCapability::ReceiveChatEvents) {
                let evt = BotToPlugin::ChatMessage {
                    platform: platform.to_string(),
                    channel: channel.to_string(),
                    user: user.to_string(),
                    text: text.to_string(),
                };
                let _ = p.send(evt).await;
            }
        }
    }

    /// Subscribe to the EventBus to watch for ChatMessages, etc.
    pub fn subscribe_to_event_bus(&self, bus: Arc<EventBus>) {
        let mut rx = bus.subscribe(None);
        let mut shutdown_rx = bus.shutdown_rx.clone();
        let pm_clone = self.clone();

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    maybe_event = rx.recv() => {
                        match maybe_event {
                            Some(event) => {
                                match event {
                                    BotEvent::ChatMessage { platform, channel, user, text, .. } => {
                                        pm_clone.handle_chat_event(&platform, &channel, &user, &text).await;
                                    }
                                    BotEvent::Tick => {
                                        // Possibly broadcast Tick to plugins
                                        // pm_clone.broadcast(BotToPlugin::Tick).await;
                                    }
                                    BotEvent::SystemMessage(msg) => {
                                        info!("(EventBus) SystemMessage -> {}", msg);
                                    }
                                }
                            },
                            None => {
                                info!("PluginManager subscriber ended (channel closed).");
                                break;
                            }
                        }
                    },
                    Ok(_) = shutdown_rx.changed() => {
                        if *shutdown_rx.borrow() {
                            info!("PluginManager subscriber shutting down (EventBus).");
                            break;
                        }
                    }
                }
            }
        });
    }

    /// Broadcast an event to ALL connected plugins (example).
    pub async fn broadcast(&self, event: BotToPlugin) {
        let plugins = self.plugins.lock().await;
        for plugin_conn in plugins.iter() {
            let _ = plugin_conn.send(event.clone()).await;
        }
    }

    /// Build a StatusResponse message for plugin or TUI usage
    pub async fn build_status_response(&self) -> BotToPlugin {
        let plugins = self.plugins.lock().await;
        let mut connected = vec![];
        for p in plugins.iter() {
            let i = p.info().await;
            connected.push(i.name);
        }
        let uptime = self.start_time.elapsed().as_secs();
        BotToPlugin::StatusResponse {
            connected_plugins: connected,
            server_uptime: uptime,
        }
    }
}
