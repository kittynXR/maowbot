// src/plugins/manager.rs

use std::any::Any;
use std::fs;
use std::net::SocketAddr;
use std::path::Path;
use std::pin::Pin;
use std::sync::Arc;

use async_trait::async_trait;
use futures_core::Stream;
use futures_util::StreamExt;
use libloading::{Library, Symbol};
use tokio::sync::Mutex;
use tokio_stream::wrappers::UnboundedReceiverStream;
use tonic::{Request, Response, Status};
use tracing::{error, info};

use crate::Error;
use crate::eventbus::{BotEvent, EventBus};
use crate::plugins::bot_api::{BotApi, StatusData};

use maowbot_proto::plugs::{
    // Tonic server trait + request/response messages:
    plugin_service_server::{PluginService, PluginServiceServer},
    PluginStreamRequest, PluginStreamResponse,
    plugin_stream_request::Payload as ReqPayload,
    plugin_stream_response::Payload as RespPayload,
    // Specific message structs/enums:
    Hello, LogMessage, RequestStatus, RequestCaps, Shutdown,
    SwitchScene, SendChat, WelcomeResponse, AuthError,
    Tick, ChatMessage, StatusResponse, CapabilityResponse,
    ForceDisconnect, PluginCapability,
};

/// Holds info about a connected plugin: name and capabilities.
#[derive(Clone)]
pub struct PluginConnectionInfo {
    pub name: String,
    pub capabilities: Vec<PluginCapability>,
}

/// Trait that all plugin connections (gRPC or in‐process) must implement.
#[async_trait]
pub trait PluginConnection: Send + Sync {
    async fn info(&self) -> PluginConnectionInfo;
    async fn set_capabilities(&self, capabilities: Vec<PluginCapability>);
    async fn set_name(&self, new_name: String);
    /// Send a Protobuf message to the plugin.
    async fn send(&self, response: PluginStreamResponse) -> Result<(), Error>;
    /// Stop the connection (force disconnect, cleanup, etc.)
    async fn stop(&self) -> Result<(), Error>;
    fn as_any(&self) -> &dyn Any;

    fn set_bot_api(&self, _api: Arc<dyn BotApi>) {
        // default: do nothing
    }
}

/// A gRPC-based plugin connection.
pub struct PluginGrpcConnection {
    info: Arc<Mutex<PluginConnectionInfo>>,
    sender: tokio::sync::mpsc::UnboundedSender<PluginStreamResponse>,
}

impl PluginGrpcConnection {
    pub fn new(sender: tokio::sync::mpsc::UnboundedSender<PluginStreamResponse>) -> Self {
        let info = PluginConnectionInfo {
            name: "<uninitialized-grpc-plugin>".to_string(),
            capabilities: Vec::new(),
        };
        Self {
            info: Arc::new(Mutex::new(info)),
            sender,
        }
    }
}

#[async_trait]
impl PluginConnection for PluginGrpcConnection {
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
        self.sender
            .send(response)
            .map_err(|_| Error::Platform("Failed to send gRPC message.".to_owned()))
    }

    async fn stop(&self) -> Result<(), Error> {
        let msg = PluginStreamResponse {
            payload: Some(RespPayload::ForceDisconnect(ForceDisconnect {
                reason: "Manager stopping connection".into(),
            })),
        };
        let _ = self.send(msg).await;
        Ok(())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// The PluginManager holds all active plugin connections, an optional passphrase for authentication,
/// and an optional reference to an event bus.
#[derive(Clone)]
pub struct PluginManager {
    pub plugins: Arc<Mutex<Vec<Arc<dyn PluginConnection>>>>,
    pub passphrase: Option<String>,
    pub start_time: std::time::Instant,
    pub event_bus: Option<Arc<EventBus>>,
}

impl PluginManager {
    pub fn new(passphrase: Option<String>) -> Self {
        Self {
            plugins: Arc::new(Mutex::new(Vec::new())),
            passphrase,
            start_time: std::time::Instant::now(),
            event_bus: None,
        }
    }

    pub fn set_event_bus(&mut self, bus: Arc<EventBus>) {
        self.event_bus = Some(bus);
    }

    /// Returns a list of connected plugin names.
    pub async fn plugin_list(&self) -> Vec<String> {
        let lock = self.plugins.lock().await;
        let mut out = Vec::new();
        for p in lock.iter() {
            out.push(p.info().await.name);
        }
        out
    }

    /// Adds a new plugin connection.
    pub async fn add_plugin_connection(&self, plugin: Arc<dyn PluginConnection>) {
        let mut lock = self.plugins.lock().await;
        lock.push(plugin);
    }

    /// Removes a plugin connection.
    pub async fn remove_plugin_connection(&self, plugin: &Arc<dyn PluginConnection>) {
        let info = plugin.info().await;
        let mut lock = self.plugins.lock().await;
        lock.retain(|p| {
            // Using block_on here is safe because we are in an async context.
            let pi = futures_lite::future::block_on(p.info());
            pi.name != info.name
        });
        info!("Removed plugin connection '{}'", info.name);
    }

    /// Subscribes to the event bus for chat events, ticks, and system messages.
    pub async fn subscribe_to_event_bus(&self, bus: Arc<super::super::eventbus::EventBus>) {
        let mut rx = bus.subscribe(None).await;
        let mut shutdown_rx = bus.shutdown_rx.clone();
        let pm_clone = self.clone();

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    maybe_event = rx.recv() => {
                        match maybe_event {
                            Some(event) => match event {
                                super::super::eventbus::BotEvent::ChatMessage { platform, channel, user, text, .. } => {
                                    pm_clone.handle_chat_event(&platform, &channel, &user, &text).await;
                                },
                                super::super::eventbus::BotEvent::Tick => {
                                    let tick_msg = PluginStreamResponse {
                                        payload: Some(RespPayload::Tick(maowbot_proto::plugs::Tick {})),
                                    };
                                    pm_clone.broadcast(tick_msg).await;
                                },
                                super::super::eventbus::BotEvent::SystemMessage(msg) => {
                                    info!("(EventBus) SystemMessage => {}", msg);
                                }
                            },
                            None => {
                                info!("PluginManager unsubscribed (channel closed).");
                                break;
                            }
                        }
                    },
                    Ok(_) = shutdown_rx.changed() => {
                        if *shutdown_rx.borrow() {
                            info!("PluginManager sees shutdown => exit loop");
                            break;
                        }
                    }
                }
            }
        });
    }

    async fn handle_chat_event(&self, platform: &str, channel: &str, user: &str, text: &str) {
        info!(
            "PluginManager: chat event => {}#{} user={} text={}",
            platform, channel, user, text
        );
        let plugins = self.plugins.lock().await;
        for plugin in plugins.iter() {
            let pi = plugin.info().await;
            if pi.capabilities.contains(&PluginCapability::ReceiveChatEvents) {
                let msg = PluginStreamResponse {
                    payload: Some(RespPayload::ChatMessage(ChatMessage {
                        platform: platform.to_string(),
                        channel: channel.to_string(),
                        user: user.to_string(),
                        text: text.to_string(),
                    })),
                };
                let _ = plugin.send(msg).await;
            }
        }
    }

    /// Broadcasts a PluginStreamResponse to all connections.
    pub async fn broadcast(&self, response: PluginStreamResponse) {
        let plugins = self.plugins.lock().await;
        for p in plugins.iter() {
            let _ = p.send(response.clone()).await;
        }
    }

    /// Loads an in-process plugin from a dynamic library file.
    ///
    /// The library must export an extern "C" function named `create_plugin` that returns a
    /// raw pointer to a boxed `dyn PluginConnection`. To ensure the library stays loaded,
    /// its handle is intentionally leaked.
    pub async fn load_in_process_plugin(&self, lib_path: &str) -> Result<(), Error> {
        if !Path::new(lib_path).exists() {
            return Err(Error::Platform(format!("Plugin library not found: {}", lib_path)));
        }

        // Since we are calling this from an async context (inside run_server),
        // simply call the plugin's constructor directly.
        unsafe {
            let lib = Library::new(lib_path)?;
            let constructor: Symbol<unsafe extern "C" fn() -> *mut (dyn PluginConnection)> =
                lib.get(b"create_plugin")?;
            let raw = constructor();
            let plugin = Arc::from_raw(raw);
            // Prevent the library from unloading by leaking its handle.
            std::mem::forget(lib);
            let mut lock = self.plugins.lock().await;
            lock.push(plugin);
        }
        Ok(())
    }

    /// Scans the specified folder for dynamic libraries and attempts to load them.
    ///
    /// This function looks for files with the appropriate extension for the current platform:
    /// - Windows: `dll`
    /// - Linux: `so`
    /// - macOS: `dylib`
    pub async fn load_plugins_from_folder(&self, folder: &str) -> Result<(), Error> {
        if !Path::new(folder).exists() {
            info!("Plugin folder '{}' does not exist; skipping.", folder);
            return Ok(());
        }

        for entry in fs::read_dir(folder)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() {
                #[cfg(target_os = "windows")]
                let expected_ext = "dll";
                #[cfg(target_os = "linux")]
                let expected_ext = "so";
                #[cfg(target_os = "macos")]
                let expected_ext = "dylib";

                if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                    if ext.eq_ignore_ascii_case(expected_ext) {
                        let path_str = path.to_str().unwrap();
                        match self.load_in_process_plugin(path_str).await {
                            Ok(_) => info!("Loaded plugin from {}", path_str),
                            Err(e) => error!("Failed to load plugin from {}: {:?}", path_str, e),
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// Builds a StatusResponse message and wraps it as a PluginStreamResponse.
    pub async fn build_status_response(&self) -> PluginStreamResponse {
        let connected = self.plugin_list().await;
        let uptime = self.start_time.elapsed().as_secs();
        PluginStreamResponse {
            payload: Some(RespPayload::StatusResponse(StatusResponse {
                connected_plugins: connected,
                server_uptime: uptime,
            })),
        }
    }

    /// Evaluates requested capabilities (given as i32) and applies policy.
    fn evaluate_caps(&self, requested: &[i32]) -> (Vec<PluginCapability>, Vec<PluginCapability>) {
        let mut granted = Vec::new();
        let mut denied = Vec::new();

        for &c in requested {
            let cap = match c {
                0 => PluginCapability::ReceiveChatEvents,
                1 => PluginCapability::SendChat,
                2 => PluginCapability::SceneManagement,
                3 => PluginCapability::ChatModeration,
                _ => PluginCapability::ReceiveChatEvents, // fallback
            };
            // Example policy: deny ChatModeration.
            if cap == PluginCapability::ChatModeration {
                denied.push(cap);
            } else {
                granted.push(cap);
            }
        }
        (granted, denied)
    }

    /// Handles inbound PluginStreamRequest messages from plugins.
    pub async fn on_inbound_message(
        &self,
        payload: ReqPayload,
        plugin: Arc<dyn PluginConnection>,
    ) {
        match payload {
            ReqPayload::Hello(Hello { plugin_name, passphrase }) => {
                if let Some(req_pass) = &self.passphrase {
                    if passphrase != *req_pass {
                        let err_resp = PluginStreamResponse {
                            payload: Some(RespPayload::AuthError(AuthError {
                                reason: "Invalid passphrase".into(),
                            })),
                        };
                        let _ = plugin.send(err_resp).await;
                        let _ = plugin.stop().await;
                        return;
                    }
                }
                plugin.set_name(plugin_name).await;
                let welcome = PluginStreamResponse {
                    payload: Some(RespPayload::Welcome(WelcomeResponse {
                        bot_name: "MaowBot".into(),
                    })),
                };
                let _ = plugin.send(welcome).await;
            }
            ReqPayload::LogMessage(LogMessage { text }) => {
                let pi = plugin.info().await;
                info!("[PLUGIN LOG: {}] {}", pi.name, text);
            }
            ReqPayload::RequestStatus(_) => {
                let status = self.build_status_response().await;
                let _ = plugin.send(status).await;
            }
            ReqPayload::RequestCaps(RequestCaps { requested }) => {
                let (granted, denied) = self.evaluate_caps(&requested);
                plugin.set_capabilities(granted.clone()).await;
                let caps = PluginStreamResponse {
                    payload: Some(RespPayload::CapabilityResponse(CapabilityResponse {
                        granted: granted.into_iter().map(|c| c as i32).collect(),
                        denied: denied.into_iter().map(|c| c as i32).collect(),
                    })),
                };
                let _ = plugin.send(caps).await;
            }
            ReqPayload::Shutdown(_) => {
                let pi = plugin.info().await;
                info!("Plugin '{}' requests entire bot shutdown!", pi.name);
                if let Some(bus) = &self.event_bus {
                    bus.shutdown();
                }
            }
            ReqPayload::SwitchScene(SwitchScene { scene_name }) => {
                let pi = plugin.info().await;
                if pi.capabilities.contains(&PluginCapability::SceneManagement) {
                    info!("Plugin '{}' switching scene => {}", pi.name, scene_name);
                } else {
                    let err = PluginStreamResponse {
                        payload: Some(RespPayload::AuthError(AuthError {
                            reason: "No SceneManagement capability".into(),
                        })),
                    };
                    let _ = plugin.send(err).await;
                }
            }
            ReqPayload::SendChat(SendChat { channel, text }) => {
                let pi = plugin.info().await;
                if pi.capabilities.contains(&PluginCapability::SendChat) {
                    info!(
                        "(PLUGIN->BOT) {} => channel='{}' => '{}'",
                        pi.name, channel, text
                    );
                    if let Some(bus) = &self.event_bus {
                        let evt = BotEvent::ChatMessage {
                            platform: "plugin".to_string(),
                            channel,
                            user: pi.name,
                            text,
                            timestamp: chrono::Utc::now(),
                        };
                        bus.publish(evt).await;
                    }
                } else {
                    let err = PluginStreamResponse {
                        payload: Some(RespPayload::AuthError(AuthError {
                            reason: "No SendChat capability".into(),
                        })),
                    };
                    let _ = plugin.send(err).await;
                }
            }
        }
    }
}

/// The Tonic gRPC service that wraps PluginManager.
#[derive(Clone)]
pub struct PluginServiceGrpc {
    pub manager: Arc<PluginManager>,
}

type SessionStream = Pin<Box<dyn Stream<Item = Result<PluginStreamResponse, Status>> + Send + 'static>>;

#[tonic::async_trait]
impl PluginService for PluginServiceGrpc {
    type StartSessionStream = SessionStream;

    async fn start_session(
        &self,
        request: Request<tonic::Streaming<PluginStreamRequest>>,
    ) -> Result<Response<Self::StartSessionStream>, Status> {
        info!("PluginServiceGrpc::start_session => new plugin stream connected.");

        let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<PluginStreamResponse>();
        let conn: Arc<dyn PluginConnection> = Arc::new(PluginGrpcConnection::new(tx));
        self.manager.add_plugin_connection(conn.clone()).await;

        let out_stream = UnboundedReceiverStream::new(rx).map(Ok);
        let pinned: Self::StartSessionStream = Box::pin(out_stream);

        let mut inbound = request.into_inner();
        let mgr_clone = self.manager.clone();
        let conn_clone = conn.clone();

        tokio::spawn(async move {
            while let Some(Ok(req)) = inbound.next().await {
                if let Some(payload) = req.payload {
                    mgr_clone.on_inbound_message(payload, conn_clone.clone()).await;
                }
            }
            info!("gRPC inbound ended => removing plugin connection");
            mgr_clone.remove_plugin_connection(&conn_clone).await;
        });

        Ok(Response::new(pinned))
    }
}

impl BotApi for PluginManager {
    fn list_plugins(&self) -> Vec<String> {
        // We do a small async call to get plugin_list:
        let fut = async { self.plugin_list().await };
        futures_lite::future::block_on(fut)
    }

    fn status(&self) -> StatusData {
        let plugin_names = self.list_plugins();
        let uptime_secs = self.start_time.elapsed().as_secs();
        StatusData {
            connected_plugins: plugin_names,
            uptime_seconds: uptime_secs,
        }
    }

    fn shutdown(&self) {
        // for example, tell the event bus to shut down:
        if let Some(bus) = &self.event_bus {
            bus.shutdown();
        }
    }
}