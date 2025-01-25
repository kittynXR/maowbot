use super::protocol::{BotToPlugin, PluginToBot};
use super::capabilities::{PluginCapability, GrantedCapabilities};
use crate::Error;
use serde_json;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::net::TcpListener;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio_rustls::rustls::{Certificate, PrivateKey, ServerConfig}; // from tokio-rustls
use tokio_rustls::TlsAcceptor;
use tracing::{error, info};

use crate::eventbus::{EventBus, BotEvent}; // <-- NEW
use tokio::sync::mpsc;

/// Represents one connected plugin: either TCP-based or in-process dynamic library.
#[derive(Clone)]
pub struct PluginConnectionInfo {
    pub name: String,
    pub capabilities: Vec<PluginCapability>,
}

/// ...
#[async_trait::async_trait]
pub trait PluginConnection: Send + Sync {
    fn info(&self) -> PluginConnectionInfo;
    fn set_capabilities(&self, capabilities: Vec<PluginCapability>);
    fn send(&self, event: BotToPlugin) -> Result<(), Error>;
    async fn stop(&self) -> Result<(), Error>;

    // ADD THIS so we can do downcast_ref in tests
    fn as_any(&self) -> &dyn std::any::Any;
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

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[async_trait::async_trait]
impl PluginConnection for TcpPluginConnection {
    fn info(&self) -> PluginConnectionInfo {
        self.info.lock().unwrap().clone()
    }

    fn set_capabilities(&self, capabilities: Vec<PluginCapability>) {
        self.info.lock().unwrap().capabilities = capabilities;
    }

    fn send(&self, event: BotToPlugin) -> Result<(), Error> {
        self.sender
            .send(event)
            .map_err(|_| Error::Platform("Failed to send to plugin channel".into()))
    }

    async fn stop(&self) -> Result<(), Error> {
        let _ = self.send(BotToPlugin::ForceDisconnect {
            reason: "Manager stopping connection".to_string(),
        });
        Ok(())
    }

    fn as_any(&self) -> &dyn std::any::Any {
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

#[async_trait::async_trait]
impl PluginConnection for DynamicPluginConnection {
    fn info(&self) -> PluginConnectionInfo {
        self.info.lock().unwrap().clone()
    }

    fn set_capabilities(&self, capabilities: Vec<PluginCapability>) {
        self.info.lock().unwrap().capabilities = capabilities;
    }

    fn send(&self, event: BotToPlugin) -> Result<(), Error> {
        info!("(InProcess) sending event to dynamic plugin: {:?}", event);
        Ok(())
    }

    async fn stop(&self) -> Result<(), Error> {
        info!("(InProcess) stopping dynamic plugin...");
        Ok(())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// PluginManager holds all active plugin connections, plus the passphrase (if any).
#[derive(Clone)]
pub struct PluginManager {
    pub plugins: Arc<Mutex<Vec<Arc<dyn PluginConnection>>>>,
    passphrase: Option<String>,
    start_time: std::time::Instant,
    // We'll also store an optional EventBus here so we can subscribe to events
    event_bus: Option<Arc<EventBus>>,
}

impl PluginManager {
    /// Create a new PluginManager.
    pub fn new(passphrase: Option<String>) -> Self {
        Self {
            plugins: Arc::new(Mutex::new(Vec::new())),
            passphrase,
            start_time: std::time::Instant::now(),
            event_bus: None,
        }
    }

    pub fn plugin_list(&self) -> std::sync::MutexGuard<'_, Vec<Arc<dyn PluginConnection>>> {
        self.plugins.lock().unwrap()
    }
    /// Assign an EventBus to the manager. We'll call this from main once we create it.
    pub fn set_event_bus(&mut self, bus: Arc<EventBus>) {
        self.event_bus = Some(bus);
    }

    /// Load an in-process plugin from a shared library or DLL.
    pub fn load_in_process_plugin(&self, path: &str) -> Result<(), Error> {
        let dynamic = DynamicPluginConnection::load_dynamic_plugin(path)?;
        self.plugins.lock().unwrap().push(Arc::new(dynamic));
        Ok(())
    }

    /// Start listening for plugin TCP (plaintext).
    pub async fn listen(&self, addr: &str) -> Result<(), Error> {
        let listener = TcpListener::bind(addr).await
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

    /// (Optional) TLS-based listen
    pub async fn listen_secure(&self, addr: &str, cert_path: &str, key_path: &str) -> Result<(), Error> {
        // same as before ...
        #![allow(unused)] // to keep the snippet clean
        unimplemented!()
    }

    /// Internal method to handle one incoming TCP plugin connection
    async fn handle_tcp_connection<T>(&self, stream: T) -> Result<(), Error>
    where
        T: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static
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

        // Validate passphrase if needed
        let hello = match serde_json::from_str::<PluginToBot>(&line) {
            Ok(PluginToBot::Hello { plugin_name, passphrase }) => {
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

        let tcp_plugin = TcpPluginConnection::new(hello.clone(), tx.clone());
        let plugin_arc = Arc::new(tcp_plugin);

        {
            let mut plugins = self.plugins.lock().unwrap();
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
        let plugin_name_clone = hello.clone();
        tokio::spawn(async move {
            while let Ok(Some(line)) = lines.next_line().await {
                match serde_json::from_str::<PluginToBot>(&line) {
                    Ok(msg) => {
                        pm_clone.on_plugin_message(msg, &plugin_name_clone, plugin_arc.clone()).await;
                    }
                    Err(e) => {
                        error!("Invalid JSON from plugin {}: {} line={}", plugin_name_clone, e, line);
                    }
                }
            }
            info!("Plugin '{}' read loop ended.", plugin_name_clone);

            // remove from manager
            let mut plugins = pm_clone.plugins.lock().unwrap();
            if let Some(idx) = plugins.iter().position(|p| p.info().name == plugin_name_clone) {
                plugins.remove(idx);
            }
        });

        // Outbound write loop
        tokio::spawn(async move {
            while let Some(evt) = rx.recv().await {
                let out = serde_json::to_string(&evt).unwrap_or_else(|_| "{\"error\":\"serialize\"}".to_string());
                if writer.write_all((out + "\n").as_bytes()).await.is_err() {
                    error!("Error writing to plugin. Possibly disconnected.");
                    break;
                }
            }
            info!("Plugin '{}' write loop ended.", hello);
        });

        Ok(())
    }

    /// Called whenever a plugin sends a `PluginToBot` message to us.
    async fn on_plugin_message(
        &self,
        message: PluginToBot,
        plugin_name: &str,
        plugin_conn: Arc<dyn PluginConnection>
    ) {
        match message {
            PluginToBot::LogMessage { text } => {
                info!("[PLUGIN LOG: {}] {}", plugin_name, text);
            }
            PluginToBot::RequestStatus => {
                // respond with info about connected plugins
                let status = self.build_status_response();
                let _ = plugin_conn.send(status);
            }
            PluginToBot::Shutdown => {
                // ...
                info!("Plugin '{}' requests a bot shutdown. (not implemented)", plugin_name);
            }
            PluginToBot::SwitchScene { scene_name } => {
                if plugin_conn.info().capabilities.contains(&PluginCapability::SceneManagement) {
                    info!("Plugin '{}' requests scene switch: {}", plugin_name, scene_name);
                } else {
                    let err = BotToPlugin::AuthError { reason: "No SceneManagement capability.".into() };
                    let _ = plugin_conn.send(err);
                }
            }
            PluginToBot::SendChat { channel, text } => {
                if plugin_conn.info().capabilities.contains(&PluginCapability::SendChat) {
                    info!("(PLUGIN->BOT) {} says: send chat to {} -> {}", plugin_name, channel, text);
                    // Possibly re-publish to EventBus as a new ChatMessage from "plugin"
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
                    let err = BotToPlugin::AuthError { reason: "No SendChat capability.".into() };
                    let _ = plugin_conn.send(err);
                }
            }
            PluginToBot::RequestCapabilities(req) => {
                info!("Plugin '{}' requests capabilities: {:?}", plugin_name, req.requested);
                let (granted, denied) = self.evaluate_capabilities(&req.requested);
                plugin_conn.set_capabilities(granted.clone());
                let response = BotToPlugin::CapabilityResponse(GrantedCapabilities { granted, denied });
                let _ = plugin_conn.send(response);
            }
            PluginToBot::Hello { .. } => {
                error!("Plugin '{}' sent Hello again unexpectedly.", plugin_name);
            }
        }
    }

    /// Evaluate which capabilities we grant or deny.
    fn evaluate_capabilities(
        &self,
        requested: &[PluginCapability]
    ) -> (Vec<PluginCapability>, Vec<PluginCapability>) {
        // For demonstration, we deny ChatModeration
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

    /// Example method to handle the ChatMessage event from the bus
    /// (See `subscribe_to_event_bus` below).
    async fn handle_chat_event(&self, platform: &str, channel: &str, user: &str, text: &str) {
        info!("PluginManager: handle_chat_event => {} #{} (user={}, text={})",
          platform, channel, user, text);
        let plugins = self.plugins.lock().unwrap();
        for p in plugins.iter() {
            let p_info = p.info();
            info!("Checking plugin '{}' with capabilities={:?}", p_info.name, p_info.capabilities);
            if p_info.capabilities.contains(&PluginCapability::ReceiveChatEvents) {
                info!("Sending chat to '{}'", p_info.name);
                let evt = BotToPlugin::ChatMessage {
                    platform: platform.to_string(),
                    channel: channel.to_string(),
                    user: user.to_string(),
                    text: text.to_string(),
                };
                let _ = p.send(evt);
            }
        }
    }


    /// Subscribe to the EventBus in order to watch for ChatMessages, etc.
    /// This is called once from your main or server init.
    pub fn subscribe_to_event_bus(&self, bus: Arc<EventBus>) {
        // 1) Create a receiver for events and a receiver for shutdown signals
        let mut rx = bus.subscribe(None); // use default buffer
        let mut shutdown_rx = bus.shutdown_rx.clone();
        let pm_clone = self.clone();

        tokio::spawn(async move {
            // 2) Use a loop with `tokio::select!` to handle both events and shutdown
            loop {
                tokio::select! {
                // Attempt to read next event from the bus
                maybe_event = rx.recv() => {
                    match maybe_event {
                        Some(event) => {
                            match event {
                                BotEvent::ChatMessage { platform, channel, user, text, .. } => {
                                    pm_clone.handle_chat_event(&platform, &channel, &user, &text).await;
                                }
                                BotEvent::Tick => {
                                    // Possibly broadcast Tick to plugins
                                    // pm_clone.broadcast(BotToPlugin::Tick);
                                }
                                BotEvent::SystemMessage(msg) => {
                                    info!("(EventBus) SystemMessage -> {}", msg);
                                }
                            }
                        },
                        None => {
                            // The channel closed => subscriber task can end
                            info!("PluginManager subscriber to EventBus ended (channel closed).");
                            break;
                        }
                    }
                },

                // Meanwhile, if our shutdown watch gets triggered, we can exit
                Ok(_) = shutdown_rx.changed() => {
                    if *shutdown_rx.borrow() {
                        info!("PluginManager subscriber shutting down due to event bus shutdown.");
                        break;
                    }
                }
            }
            }
        });
    }


    /// Build a StatusResponse message
    fn build_status_response(&self) -> BotToPlugin {
        let plugins = self.plugins.lock().unwrap();
        let connected: Vec<String> = plugins.iter().map(|p| p.info().name.clone()).collect();
        let uptime = self.start_time.elapsed().as_secs();
        BotToPlugin::StatusResponse {
            connected_plugins: connected,
            server_uptime: uptime,
        }
    }

    /// Broadcast an event to ALL connected plugins (legacy approach).
    pub fn broadcast(&self, event: BotToPlugin) {
        let plugins = self.plugins.lock().unwrap();
        for plugin_conn in plugins.iter() {
            let _ = plugin_conn.send(event.clone());
        }
    }
}
