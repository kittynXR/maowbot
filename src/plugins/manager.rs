// src/plugins/manager.rs

use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{error, info};
use async_trait::async_trait;
use std::any::Any;
use std::pin::Pin;

use crate::Error;
use crate::eventbus::{EventBus, BotEvent};

use crate::plugins::proto::plugs::{
    // Tonic server trait + request/response messages:
    plugin_service_server::{PluginService, PluginServiceServer},
    PluginStreamRequest, PluginStreamResponse,
    plugin_stream_request::Payload as ReqPayload,
    plugin_stream_response::Payload as RespPayload,

    // Specific message structs/enums from plugin.proto:
    Hello, LogMessage, RequestStatus, RequestCaps, Shutdown,
    SwitchScene, SendChat, WelcomeResponse, AuthError,
    Tick, ChatMessage, StatusResponse, CapabilityResponse,
    ForceDisconnect, PluginCapability,
};

use futures_core::Stream;
use tokio_stream::{wrappers::UnboundedReceiverStream};
use futures_util::StreamExt;
use tonic::{Request, Response, Status};

/// Holds info about a connected plugin: name + capabilities from the .proto enum.
#[derive(Clone)]
pub struct PluginConnectionInfo {
    pub name: String,
    pub capabilities: Vec<PluginCapability>, // direct from proto
}

/// Trait that all plugin connections implement (in-process or gRPC).
#[async_trait]
pub trait PluginConnection: Send + Sync {
    async fn info(&self) -> PluginConnectionInfo;
    async fn set_capabilities(&self, capabilities: Vec<PluginCapability>);

    async fn set_name(&self, new_name: String);

    /// Send a **Protobuf** PluginStreamResponse to the plugin
    async fn send(&self, response: PluginStreamResponse) -> Result<(), Error>;

    /// Stop the connection (e.g. force a disconnect)
    async fn stop(&self) -> Result<(), Error>;

    fn as_any(&self) -> &dyn Any;
}

/// A gRPC-based plugin connection (used by `PluginServiceGrpc`).
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
        // Optionally send a ForceDisconnect to the plugin
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

/// An in-process plugin connection example. Still uses the same trait; we keep it simple.
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

    async fn set_name(&self, new_name: String) {
        let mut guard = self.info.lock().await;
        guard.name = new_name;
    }

    async fn send(&self, response: PluginStreamResponse) -> Result<(), Error> {
        // For an in-process plugin, we might just log the response:
        if let Some(payload) = response.payload {
            info!("(DynamicPlugin) got payload => {:?}", payload);
        }
        Ok(())
    }

    async fn stop(&self) -> Result<(), Error> {
        info!("(DynamicPlugin) plugin stopping...");
        Ok(())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// The PluginManager holds all active plugin connections and optional passphrase for auth.
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

    /// Return a list of connected plugin names
    pub async fn plugin_list(&self) -> Vec<String> {
        let lock = self.plugins.lock().await;
        let mut out = Vec::new();
        for p in lock.iter() {
            out.push(p.info().await.name);
        }
        out
    }

    /// Add a new plugin connection
    pub async fn add_plugin_connection(&self, plugin: Arc<dyn PluginConnection>) {
        let mut lock = self.plugins.lock().await;
        lock.push(plugin);
    }

    /// Remove a plugin connection
    pub async fn remove_plugin_connection(&self, plugin: &Arc<dyn PluginConnection>) {
        let info = plugin.info().await;
        let mut lock = self.plugins.lock().await;
        lock.retain(|p| {
            let pi = futures_lite::future::block_on(p.info());
            pi.name != info.name
        });
        info!("Removed plugin connection '{}'", info.name);
    }

    /// Subscribe to the EventBus for chat events, etc.
    pub fn subscribe_to_event_bus(&self, bus: Arc<EventBus>) {
        let mut rx = bus.subscribe(None);
        let mut shutdown_rx = bus.shutdown_rx.clone();
        let pm_clone = self.clone();

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    maybe_event = rx.recv() => {
                        match maybe_event {
                            Some(event) => match event {
                                BotEvent::ChatMessage { platform, channel, user, text, .. } => {
                                    pm_clone.handle_chat_event(&platform, &channel, &user, &text).await;
                                },
                                BotEvent::Tick => {
                                    // broadcast Tick to all plugins
                                    let tick_msg = PluginStreamResponse {
                                        payload: Some(RespPayload::Tick(Tick{})),
                                    };
                                    pm_clone.broadcast(tick_msg).await;
                                },
                                BotEvent::SystemMessage(msg) => {
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
        info!("PluginManager: chat event => {}#{} user={} text={}", platform, channel, user, text);

        let plugins = self.plugins.lock().await;
        for plugin in plugins.iter() {
            let pi = plugin.info().await;
            // only forward if plugin has the RECV capability
            if pi.capabilities.contains(&PluginCapability::ReceiveChatEvents) {
                // build a ChatMessage response
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

    /// Broadcast a single PluginStreamResponse to all connections
    pub async fn broadcast(&self, response: PluginStreamResponse) {
        let plugins = self.plugins.lock().await;
        for p in plugins.iter() {
            let _ = p.send(response.clone()).await;
        }
    }

    /// Load a dynamic in-process plugin
    pub fn load_in_process_plugin(&self, path: &str) -> Result<(), Error> {
        let plugin = DynamicPluginConnection::load_dynamic_plugin(path)?;
        let mut lock = self.plugins.blocking_lock();
        lock.push(Arc::new(plugin));
        Ok(())
    }

    /// Build a StatusResponse message as a PluginStreamResponse
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

    /// Convert requested i32 -> actual proto enum, applying any policy
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
            // Example policy: deny ChatModeration
            if cap == PluginCapability::ChatModeration {
                denied.push(cap);
            } else {
                granted.push(cap);
            }
        }
        (granted, denied)
    }

    /// Called whenever we receive an inbound `PluginStreamRequest` from the plugin's gRPC stream
    pub async fn on_inbound_message(
        &self,
        payload: ReqPayload,
        plugin: Arc<dyn PluginConnection>,
    ) {
        match payload {
            // plugin says hello
            ReqPayload::Hello(Hello { plugin_name, passphrase }) => {
                // check passphrase
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

                // send a welcome
                let welcome = PluginStreamResponse {
                    payload: Some(RespPayload::Welcome(WelcomeResponse {
                        bot_name: "MaowBot".into(),
                    })),
                };
                let _ = plugin.send(welcome).await;
            }

            // plugin log message
            ReqPayload::LogMessage(LogMessage { text }) => {
                let pi = plugin.info().await;
                info!("[PLUGIN LOG: {}] {}", pi.name, text);
            }

            // plugin wants the current status
            ReqPayload::RequestStatus(_) => {
                let status = self.build_status_response().await;
                let _ = plugin.send(status).await;
            }

            // plugin requests certain capabilities
            ReqPayload::RequestCaps(RequestCaps { requested }) => {
                let (granted, denied) = self.evaluate_caps(&requested);
                plugin.set_capabilities(granted.clone()).await;

                // build a CapabilityResponse
                let caps = PluginStreamResponse {
                    payload: Some(RespPayload::CapabilityResponse(CapabilityResponse {
                        granted: granted.into_iter().map(|c| c as i32).collect(),
                        denied: denied.into_iter().map(|c| c as i32).collect(),
                    })),
                };
                let _ = plugin.send(caps).await;
            }

            // plugin wants the bot to shut down
            ReqPayload::Shutdown(_) => {
                let pi = plugin.info().await;
                info!("Plugin '{}' requests entire bot shutdown!", pi.name);
                if let Some(bus) = &self.event_bus {
                    bus.shutdown();
                }
            }

            // plugin wants to switch scene
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

            // plugin wants to send chat
            ReqPayload::SendChat(SendChat { channel, text }) => {
                let pi = plugin.info().await;
                if pi.capabilities.contains(&PluginCapability::SendChat) {
                    info!("(PLUGIN->BOT) {} => channel='{}' => '{}'",
                          pi.name, channel, text);

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

/// The actual Tonic gRPC service. Notice we have `start_session(...)` instead of `connect(...)`.
#[derive(Clone)]
pub struct PluginServiceGrpc {
    pub manager: Arc<PluginManager>,
}

type SessionStream = Pin<
    Box<dyn Stream<Item = Result<PluginStreamResponse, Status>> + Send + 'static>
>;

#[tonic::async_trait]
impl PluginService for PluginServiceGrpc {
    type StartSessionStream = SessionStream;

    async fn start_session(
        &self,
        request: Request<tonic::Streaming<PluginStreamRequest>>,
    ) -> Result<Response<Self::StartSessionStream>, Status> {
        info!("PluginServiceGrpc::start_session => new plugin stream connected.");

        // 1) create an unbounded channel for server->plugin messages
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<PluginStreamResponse>();

        // 2) create a connection object
        let conn: Arc<dyn PluginConnection> = Arc::new(PluginGrpcConnection::new(tx));
        self.manager.add_plugin_connection(conn.clone()).await;

        // 3) turn `rx` into a Stream
        let out_stream = UnboundedReceiverStream::new(rx).map(Ok);
        let pinned: Self::StartSessionStream = Box::pin(out_stream);

        // 4) spawn a task that handles inbound from the plugin
        let mut inbound = request.into_inner();
        let mgr_clone = self.manager.clone();
        let conn_clone = conn.clone();

        tokio::spawn(async move {
            while let Some(Ok(req)) = inbound.next().await {
                if let Some(payload) = req.payload {
                    mgr_clone.on_inbound_message(payload, conn_clone.clone()).await;
                }
            }
            // inbound ended => remove plugin
            info!("gRPC inbound ended => removing plugin connection");
            mgr_clone.remove_plugin_connection(&conn_clone).await;
        });

        Ok(Response::new(pinned))
    }
}
