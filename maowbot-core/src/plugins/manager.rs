// maowbot-core/src/plugins/manager.rs

use std::any::Any;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use async_trait::async_trait;
use futures_core::Stream;
use futures_util::StreamExt;
use libloading::{Library, Symbol};
use serde::{Deserialize, Serialize};
use tokio::sync::{Mutex as AsyncMutex, mpsc::UnboundedSender};
use tokio_stream::wrappers::UnboundedReceiverStream;
use tonic::{Request, Response, Status};
use tracing::{error, info};

use crate::Error;
use crate::eventbus::{BotEvent, EventBus};
use crate::plugins::bot_api::{
    BotApi,
    StatusData,
    PlatformConfigData,
};
use maowbot_proto::plugs::{
    plugin_service_server::{PluginService, PluginServiceServer},
    plugin_stream_request::{Payload as ReqPayload},
    plugin_stream_response::{Payload as RespPayload},
    PluginStreamRequest, PluginStreamResponse,
    Hello, LogMessage, RequestStatus, RequestCaps, Shutdown,
    SwitchScene, SendChat, WelcomeResponse, AuthError,
    Tick, ChatMessage, StatusResponse, CapabilityResponse,
    ForceDisconnect, PluginCapability,
};
use crate::auth::AuthManager;
use crate::models::{Platform, PlatformCredential};

/// Plugin type.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PluginType {
    DynamicLib { path: String },
    Grpc,
}

/// Record for a plugin.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginRecord {
    pub name: String,
    pub plugin_type: PluginType,
    pub enabled: bool,
}

/// Persistent plugin state.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct PluginStatesFile {
    pub plugins: Vec<PluginRecord>,
}

/// Trait for plugin connections.
#[async_trait]
pub trait PluginConnection: Send + Sync {
    async fn info(&self) -> PluginConnectionInfo;
    async fn set_capabilities(&self, capabilities: Vec<PluginCapability>);
    async fn set_name(&self, new_name: String);
    async fn send(&self, response: PluginStreamResponse) -> Result<(), Error>;
    async fn stop(&self) -> Result<(), Error>;
    fn as_any(&self) -> &dyn Any;
    fn set_bot_api(&self, _api: Arc<dyn BotApi>) {}

    async fn set_enabled(&self, enable: bool);
}

/// In‑memory info about a plugin connection.
#[derive(Clone)]
pub struct PluginConnectionInfo {
    pub name: String,
    pub capabilities: Vec<PluginCapability>,
    pub is_enabled: bool,
}

/// A gRPC plugin connection.
pub struct PluginGrpcConnection {
    info: Arc<AsyncMutex<PluginConnectionInfo>>,
    sender: UnboundedSender<PluginStreamResponse>,
}

impl PluginGrpcConnection {
    /// Note: starts with `is_enabled=false`.
    pub fn new(sender: UnboundedSender<PluginStreamResponse>, initially_enabled: bool) -> Self {
        let info = PluginConnectionInfo {
            name: "<uninitialized-grpc-plugin>".to_string(),
            capabilities: Vec::new(),
            is_enabled: initially_enabled,
        };
        Self {
            info: Arc::new(AsyncMutex::new(info)),
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

    async fn set_enabled(&self, enable: bool) {
        let mut guard = self.info.lock().await;
        guard.is_enabled = enable;
    }
}

/// An in‑process plugin connection.
pub struct InProcessPluginConnection {
    plugin: Arc<dyn PluginConnection>,
    info: Arc<AsyncMutex<PluginConnectionInfo>>,
}

impl InProcessPluginConnection {
    pub fn new(plugin: Arc<dyn PluginConnection>, enabled: bool) -> Self {
        let info = PluginConnectionInfo {
            name: "<uninitialized-inproc-plugin>".to_string(),
            capabilities: Vec::new(),
            is_enabled: enabled,
        };
        Self {
            plugin,
            info: Arc::new(AsyncMutex::new(info)),
        }
    }
}

#[async_trait]
impl PluginConnection for InProcessPluginConnection {
    async fn info(&self) -> PluginConnectionInfo {
        let guard = self.info.lock().await;
        guard.clone()
    }

    async fn set_capabilities(&self, capabilities: Vec<PluginCapability>) {
        {
            let mut guard = self.info.lock().await;
            guard.capabilities = capabilities.clone();
        }
        self.plugin.set_capabilities(capabilities).await;
    }

    async fn set_name(&self, new_name: String) {
        {
            let mut guard = self.info.lock().await;
            guard.name = new_name.clone();
        }
        self.plugin.set_name(new_name).await;
    }

    async fn send(&self, response: PluginStreamResponse) -> Result<(), Error> {
        let guard = self.info.lock().await;
        if !guard.is_enabled {
            // If plugin is disabled, ignore sends
            return Ok(());
        }
        self.plugin.send(response).await
    }

    async fn stop(&self) -> Result<(), Error> {
        self.plugin.stop().await
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn set_bot_api(&self, api: Arc<dyn BotApi>) {
        self.plugin.set_bot_api(api);
    }

    async fn set_enabled(&self, enable: bool) {
        let mut guard = self.info.lock().await;
        guard.is_enabled = enable;
    }
}

/// The PluginManager holds active plugin connections + plugin_records (JSON).
#[derive(Clone)]
pub struct PluginManager {
    pub plugins: Arc<AsyncMutex<Vec<Arc<dyn PluginConnection>>>>,
    pub plugin_records: Arc<Mutex<Vec<PluginRecord>>>,
    pub passphrase: Option<String>,
    pub start_time: Instant,
    pub event_bus: Option<Arc<EventBus>>,
    pub persist_path: PathBuf,
    pub auth_manager: Option<Arc<tokio::sync::Mutex<AuthManager>>>,
}

impl PluginManager {
    pub fn new(passphrase: Option<String>) -> Self {
        let manager = Self {
            plugins: Arc::new(AsyncMutex::new(Vec::new())),
            plugin_records: Arc::new(Mutex::new(Vec::new())),
            passphrase,
            start_time: Instant::now(),
            event_bus: None,
            persist_path: PathBuf::from("plugs/plugins_state.json"),
            auth_manager: None,
        };
        manager.load_plugin_states();
        manager
    }

    pub fn set_auth_manager(&mut self, am: Arc<tokio::sync::Mutex<AuthManager>>) {
        self.auth_manager = Some(am);
    }

    pub fn set_event_bus(&mut self, bus: Arc<EventBus>) {
        self.event_bus = Some(bus);
    }

    fn load_plugin_states(&self) {
        if !self.persist_path.exists() {
            info!("No plugin-states file at {:?}; using empty defaults.", self.persist_path);
            return;
        }
        match fs::read_to_string(&self.persist_path) {
            Ok(contents) => {
                match serde_json::from_str::<PluginStatesFile>(&contents) {
                    Ok(parsed) => {
                        let mut lock = self.plugin_records.lock().unwrap();
                        *lock = parsed.plugins;
                        info!(
                            "Loaded {} plugin records from {:?}",
                            lock.len(),
                            self.persist_path
                        );
                    }
                    Err(e) => {
                        error!("Could not parse plugin-states JSON at {:?}: {:?}", self.persist_path, e);
                    }
                }
            }
            Err(e) => {
                error!("Could not read plugin-states file at {:?}: {:?}", self.persist_path, e);
            }
        }
    }

    fn save_plugin_states(&self) {
        let lock = self.plugin_records.lock().unwrap();
        let data = PluginStatesFile {
            plugins: lock.clone(),
        };
        let contents = match serde_json::to_string_pretty(&data) {
            Ok(s) => s,
            Err(e) => {
                error!("Failed to serialize plugin states: {:?}", e);
                return;
            }
        };
        let _ = fs::create_dir_all("plugs");
        if let Err(e) = fs::write(&self.persist_path, contents) {
            error!("Failed to write plugin-states file: {:?}", e);
        }
    }

    pub fn get_plugin_records(&self) -> Vec<PluginRecord> {
        self.plugin_records.lock().unwrap().clone()
    }

    pub async fn list_connected_plugins(&self) -> Vec<PluginConnectionInfo> {
        let lock = self.plugins.lock().await;
        let mut out = Vec::new();
        for p in lock.iter() {
            out.push(p.info().await);
        }
        out
    }

    fn upsert_plugin_record(&self, record: PluginRecord) {
        let mut lock = self.plugin_records.lock().unwrap();
        if let Some(existing) = lock.iter_mut()
            .find(|r| r.name == record.name && r.plugin_type == record.plugin_type)
        {
            existing.enabled = record.enabled;
        } else {
            lock.push(record);
        }
        drop(lock);
        self.save_plugin_states();
    }

    fn is_plugin_enabled(&self, name: &str, plugin_type: &PluginType) -> bool {
        let lock = self.plugin_records.lock().unwrap();
        lock.iter()
            .find(|r| r.name == name && r.plugin_type == *plugin_type)
            .map(|r| r.enabled)
            .unwrap_or(false)
    }

    /// Discovers a plugin record from its library path.
    pub fn discover_dynamic_plugin(&self, path_str: &str) -> PluginRecord {
        let file_stem = Path::new(path_str)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown-plugin");
        let plugin_name = file_stem.to_string();
        let plugin_type = PluginType::DynamicLib { path: path_str.to_string() };
        let enabled = self.is_plugin_enabled(&plugin_name, &plugin_type);
        PluginRecord { name: plugin_name, plugin_type, enabled }
    }

    pub async fn add_plugin_connection(&self, plugin: Arc<dyn PluginConnection>) {
        let mut lock = self.plugins.lock().await;
        lock.push(plugin);
    }

    pub async fn remove_plugin_connection(&self, plugin: &Arc<dyn PluginConnection>) {
        let info = plugin.info().await;
        let mut lock = self.plugins.lock().await;
        lock.retain(|p| {
            let pi = futures_lite::future::block_on(p.info());
            pi.name != info.name
        });
        info!("Removed plugin connection '{}'", info.name);
    }

    /// Enable/disable plugin by name.
    pub async fn toggle_plugin_async(&self, plugin_name: &str, enable: bool) -> Result<(), Error> {
        let maybe_rec = {
            let lock = self.plugin_records.lock().unwrap();
            lock.iter().find(|r| r.name == plugin_name).cloned()
        };
        let rec = match maybe_rec {
            Some(r) => r,
            None => return Err(Error::Platform(format!("No known plugin named '{}'", plugin_name))),
        };

        if rec.enabled == enable {
            return Ok(());
        }
        let updated = PluginRecord {
            name: rec.name.clone(),
            plugin_type: rec.plugin_type.clone(),
            enabled: enable,
        };
        self.upsert_plugin_record(updated.clone());
        info!(
            "PluginManager: set plugin '{}' to {}",
            updated.name,
            if enable { "ENABLED" } else { "DISABLED" },
        );

        match updated.plugin_type {
            PluginType::Grpc => {
                let lock = self.plugins.lock().await;
                for p in lock.iter() {
                    let pi = p.info().await;
                    if pi.name == updated.name {
                        p.set_capabilities(pi.capabilities.clone()).await;
                        p.set_enabled(enable).await;
                        break;
                    }
                }
            }
            PluginType::DynamicLib { .. } => {
                if enable {
                    let mut lock = self.plugins.lock().await;
                    let already_loaded = lock.iter().any(|p| {
                        let pi = futures_lite::future::block_on(p.info());
                        pi.name == updated.name
                    });
                    drop(lock);

                    if !already_loaded {
                        if let Err(e) = self.load_in_process_plugin_by_record(&updated).await {
                            error!("Failed to load '{}': {:?}", updated.name, e);
                        }
                    } else {
                        let mut lock = self.plugins.lock().await;
                        for p in lock.iter() {
                            let pi = p.info().await;
                            if pi.name == updated.name {
                                p.set_capabilities(pi.capabilities.clone()).await;
                                p.set_enabled(true).await;
                                break;
                            }
                        }
                    }
                } else {
                    let mut lock = self.plugins.lock().await;
                    if let Some(i) = lock.iter().position(|p| {
                        let pi = futures_lite::future::block_on(p.info());
                        pi.name == updated.name
                    }) {
                        let plugin_arc = lock.remove(i);
                        let _ = plugin_arc.stop().await;
                        info!("Unloaded in-process plugin '{}'", updated.name);
                    }
                }
            }
        }

        Ok(())
    }

    pub async fn load_in_process_plugin_by_record(&self, record: &PluginRecord) -> Result<(), Error> {
        let path_str = match &record.plugin_type {
            PluginType::DynamicLib { path } => path.clone(),
            _ => return Err(Error::Platform("Plugin record is not dynamic-lib type".to_string())),
        };
        if !Path::new(&path_str).exists() {
            return Err(Error::Platform(format!("Plugin library not found: {}", path_str)));
        }
        unsafe {
            let lib = Library::new(&path_str)?;
            let constructor: Symbol<unsafe extern "C" fn() -> *mut (dyn PluginConnection)> =
                lib.get(b"create_plugin")?;
            let raw = constructor();
            let plugin = Arc::from_raw(raw);
            let inproc_conn = Arc::new(InProcessPluginConnection::new(plugin, record.enabled));
            std::mem::forget(lib);
            inproc_conn.set_name(record.name.clone()).await;
            self.add_plugin_connection(inproc_conn).await;
        }
        Ok(())
    }

    /// Load a single in-process plugin from a .dll/.so path.
    pub async fn load_in_process_plugin(&self, path: &str) -> Result<(), Error> {
        let rec = self.discover_dynamic_plugin(path);
        self.upsert_plugin_record(rec.clone());
        if rec.enabled {
            self.load_in_process_plugin_by_record(&rec).await?;
        }
        Ok(())
    }

    /// Load all .dll/.so in `folder` that match this OS's extension, if they are "enabled".
    pub async fn load_plugins_from_folder(&self, folder: &str) -> Result<(), Error> {
        if !Path::new(folder).exists() {
            info!("Plugin folder '{}' does not exist; skipping.", folder);
            return Ok(());
        }

        #[cfg(target_os = "windows")]
        let ext = "dll";
        #[cfg(target_os = "linux")]
        let ext = "so";
        #[cfg(target_os = "macos")]
        let ext = "dylib";

        for entry in fs::read_dir(folder)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() {
                if let Some(ext_str) = path.extension().and_then(|s| s.to_str()) {
                    if ext_str.eq_ignore_ascii_case(ext) {
                        let path_str = path.to_string_lossy().to_string();
                        let file_stem = path
                            .file_stem()
                            .and_then(|s| s.to_str())
                            .unwrap_or("unknown");
                        let plugin_type = PluginType::DynamicLib {
                            path: path_str.clone(),
                        };
                        let enabled = self.is_plugin_enabled(file_stem, &plugin_type);
                        let rec = PluginRecord {
                            name: file_stem.to_string(),
                            plugin_type,
                            enabled,
                        };
                        self.upsert_plugin_record(rec.clone());
                        if rec.enabled {
                            if let Err(e) = self.load_in_process_plugin_by_record(&rec).await {
                                error!(
                                    "Failed to load '{}' from {}: {:?}",
                                    rec.name, path_str, e
                                );
                            } else {
                                info!("Loaded plugin '{}' from {}", rec.name, path_str);
                            }
                        } else {
                            info!(
                                "Found plugin at {}, but it's disabled => skipping load",
                                path_str
                            );
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// Called when a new gRPC plugin connects.
    pub async fn handle_new_grpc_stream(
        &self,
        mut inbound: tonic::Streaming<PluginStreamRequest>,
        sender: UnboundedSender<PluginStreamResponse>,
    ) {
        // Start off as disabled=false; we might enable it after reading the plugin name.
        let conn = Arc::new(PluginGrpcConnection::new(sender, false));
        self.add_plugin_connection(conn.clone()).await;

        let mgr_clone = self.clone();
        tokio::spawn(async move {
            while let Some(Ok(req)) = inbound.next().await {
                if let Some(payload) = req.payload {
                    mgr_clone.on_inbound_message(payload, conn.clone()).await;
                }
            }
            info!("gRPC plugin stream ended => removing plugin connection");
            let dyn_conn: Arc<dyn PluginConnection> = conn.clone();
            mgr_clone.remove_plugin_connection(&dyn_conn).await;
        });
    }

    /// Routes inbound plugin messages (Hello, LogMessage, etc.).
    pub async fn on_inbound_message(&self, payload: ReqPayload, plugin: Arc<dyn PluginConnection>) {
        match payload {
            ReqPayload::Hello(Hello { plugin_name, passphrase }) => {
                // Passphrase check:
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
                let plugin_type = PluginType::Grpc;
                let is_enabled = self.is_plugin_enabled(&plugin_name, &plugin_type);

                let rec = PluginRecord {
                    name: plugin_name.clone(),
                    plugin_type,
                    enabled: is_enabled,
                };
                self.upsert_plugin_record(rec);

                plugin.set_name(plugin_name.clone()).await;
                plugin.set_enabled(is_enabled).await;

                let welcome = PluginStreamResponse {
                    payload: Some(RespPayload::Welcome(WelcomeResponse {
                        bot_name: "MaowBot".into(),
                    })),
                };
                let _ = plugin.send(welcome).await;
            }
            ReqPayload::LogMessage(LogMessage { text }) => {
                let pi = plugin.info().await;
                if !pi.is_enabled {
                    return;
                }
                info!("[PLUGIN LOG: {}] {}", pi.name, text);
            }
            ReqPayload::RequestStatus(_) => {
                let pi = plugin.info().await;
                if !pi.is_enabled {
                    return;
                }
                let status = self.build_status_response().await;
                let _ = plugin.send(status).await;
            }
            ReqPayload::RequestCaps(RequestCaps { requested }) => {
                let pi = plugin.info().await;
                if !pi.is_enabled {
                    return;
                }
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
                if !pi.is_enabled {
                    return;
                }
                info!("Plugin '{}' requests entire bot shutdown!", pi.name);
                if let Some(bus) = &self.event_bus {
                    bus.shutdown();
                }
            }
            ReqPayload::SwitchScene(SwitchScene { scene_name }) => {
                let pi = plugin.info().await;
                if !pi.is_enabled {
                    return;
                }
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
                if !pi.is_enabled {
                    return;
                }
                if pi.capabilities.contains(&PluginCapability::SendChat) {
                    info!("(PLUGIN->BOT) {} => channel='{}' => '{}'",
                          pi.name, channel, text);
                    if let Some(bus) = &self.event_bus {
                        let evt = BotEvent::ChatMessage {
                            platform: "plugin".to_string(),
                            channel,
                            user: pi.name.clone(),
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

    fn evaluate_caps(&self, requested: &[i32]) -> (Vec<PluginCapability>, Vec<PluginCapability>) {
        let mut granted = Vec::new();
        let mut denied = Vec::new();
        for &c in requested {
            let cap = match c {
                0 => PluginCapability::ReceiveChatEvents,
                1 => PluginCapability::SendChat,
                2 => PluginCapability::SceneManagement,
                3 => PluginCapability::ChatModeration,
                _ => PluginCapability::ReceiveChatEvents,
            };
            // Example: we deny ChatModeration for untrusted plugins
            if cap == PluginCapability::ChatModeration {
                denied.push(cap);
            } else {
                granted.push(cap);
            }
        }
        (granted, denied)
    }

    pub async fn build_status_response(&self) -> PluginStreamResponse {
        let connected = {
            let infos = self.list_connected_plugins().await;
            infos.into_iter().map(|i| i.name).collect::<Vec<_>>()
        };
        let uptime = self.start_time.elapsed().as_secs();
        PluginStreamResponse {
            payload: Some(RespPayload::StatusResponse(StatusResponse {
                connected_plugins: connected,
                server_uptime: uptime,
            })),
        }
    }

    pub async fn broadcast(&self, response: PluginStreamResponse, required_cap: Option<PluginCapability>) {
        let lock = self.plugins.lock().await;
        for p in lock.iter() {
            let pi = p.info().await;
            if !pi.is_enabled {
                continue;
            }
            if let Some(cap) = &required_cap {
                if !pi.capabilities.contains(cap) {
                    continue;
                }
            }
            let _ = p.send(response.clone()).await;
        }
    }

    pub async fn subscribe_to_event_bus(&self, bus: Arc<EventBus>) {
        let mut rx = bus.subscribe(None).await;
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
                                    let tick_msg = PluginStreamResponse {
                                        payload: Some(RespPayload::Tick(Tick {})),
                                    };
                                    pm_clone.broadcast(tick_msg, None).await;
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
        let msg = PluginStreamResponse {
            payload: Some(RespPayload::ChatMessage(ChatMessage {
                platform: platform.to_string(),
                channel: channel.to_string(),
                user: user.to_string(),
                text: text.to_string(),
            })),
        };
        self.broadcast(msg, Some(PluginCapability::ReceiveChatEvents)).await;
    }

    // ---------------------------------------------------
    // remove_plugin => remove plugin from JSON + memory
    // ---------------------------------------------------
    pub async fn remove_plugin(&self, plugin_name: &str) -> Result<(), Error> {
        let maybe_record = {
            let lock = self.plugin_records.lock().unwrap();
            lock.iter().find(|r| r.name == plugin_name).cloned()
        };

        let record = match maybe_record {
            Some(r) => r,
            None => {
                return Err(Error::Platform(format!("No known plugin named '{}'", plugin_name)));
            }
        };

        // If plugin is loaded in memory, stop it & remove
        {
            let mut lock = self.plugins.lock().await;
            if let Some(pos) = lock.iter().position(|p| {
                let pi = futures_lite::future::block_on(p.info());
                pi.name == record.name
            }) {
                let plugin_arc = lock.remove(pos);
                let _ = plugin_arc.stop().await;
                info!("Stopped and removed in-memory plugin '{}'", record.name);
            }
        }

        // Remove from plugin_records
        {
            let mut lock = self.plugin_records.lock().unwrap();
            lock.retain(|r| r.name != record.name);
        }
        self.save_plugin_states();
        info!("Plugin '{}' removed from JSON records.", plugin_name);

        Ok(())
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
        let mgr = self.manager.clone();
        mgr.handle_new_grpc_stream(request.into_inner(), tx).await;

        let out_stream = UnboundedReceiverStream::new(rx).map(Ok);
        let pinned: Self::StartSessionStream = Box::pin(out_stream);
        Ok(Response::new(pinned))
    }
}

// ---------------------------------------------------------------------------
// Implementation of BotApi for PluginManager
// ---------------------------------------------------------------------------
#[async_trait]
impl BotApi for PluginManager {
    async fn list_plugins(&self) -> Vec<String> {
        let records = self.get_plugin_records();
        records
            .into_iter()
            .map(|r| {
                let suffix = if r.enabled { "" } else { " (disabled)" };
                format!("{}{}", r.name, suffix)
            })
            .collect()
    }

    async fn status(&self) -> StatusData {
        let connected = self.list_connected_plugins().await;
        let connected_names: Vec<_> = connected
            .into_iter()
            .map(|p| {
                let suffix = if p.is_enabled { "" } else { " (disabled)" };
                format!("{}{}", p.name, suffix)
            })
            .collect();

        StatusData {
            connected_plugins: connected_names,
            uptime_seconds: self.start_time.elapsed().as_secs(),
        }
    }

    async fn shutdown(&self) {
        if let Some(bus) = &self.event_bus {
            bus.shutdown();
        }
    }

    async fn toggle_plugin(&self, plugin_name: &str, enable: bool) -> Result<(), Error> {
        self.toggle_plugin_async(plugin_name, enable).await
    }

    async fn remove_plugin(&self, plugin_name: &str) -> Result<(), Error> {
        self.remove_plugin(plugin_name).await
    }

    // ---------- Auth Flow from the snippet ----------

    async fn begin_auth_flow(
        &self,
        platform: Platform,
        is_bot: bool
    ) -> Result<String, Error> {
        self.begin_auth_flow_with_label(platform, is_bot, "default").await
    }

    async fn begin_auth_flow_with_label(
        &self,
        platform: Platform,
        is_bot: bool,
        label: &str
    ) -> Result<String, Error> {
        if let Some(am) = &self.auth_manager {
            let mut lock = am.lock().await;
            lock.begin_auth_flow_with_label(platform, is_bot, label).await
        } else {
            Err(Error::Auth("No auth manager set in plugin manager".into()))
        }
    }

    async fn complete_auth_flow(
        &self,
        platform: Platform,
        code: String
    ) -> Result<PlatformCredential, Error> {
        if let Some(am) = &self.auth_manager {
            let mut lock = am.lock().await;
            lock.complete_auth_flow(platform, code).await
        } else {
            Err(Error::Auth("No auth manager set in plugin manager".into()))
        }
    }

    async fn revoke_credentials(
        &self,
        platform: Platform,
        user_id: &str
    ) -> Result<(), Error> {
        if let Some(am) = &self.auth_manager {
            let mut lock = am.lock().await;
            lock.revoke_credentials(&platform, user_id).await
        } else {
            Err(Error::Auth("No auth manager set in plugin manager".into()))
        }
    }

    async fn list_credentials(
        &self,
        maybe_platform: Option<Platform>
    ) -> Result<Vec<PlatformCredential>, Error> {
        if let Some(am) = &self.auth_manager {
            let lock = am.lock().await;
            let all = lock.credentials_repo
                .get_all_credentials()
                .await?;
            if let Some(p) = maybe_platform {
                Ok(all.into_iter().filter(|c| c.platform == p).collect())
            } else {
                Ok(all)
            }
        } else {
            Err(Error::Auth("No auth manager set in plugin manager".into()))
        }
    }

    async fn create_platform_config(
        &self,
        platform: Platform,
        label: &str,
        client_id: String,
        client_secret: Option<String>
    ) -> Result<(), Error> {
        if let Some(am) = &self.auth_manager {
            let mut lock = am.lock().await;
            let platform_str = format!("{}", platform);
            lock.create_platform_config(platform_str.as_str(), label, client_id, client_secret).await
        } else {
            Err(Error::Auth("No auth manager set in plugin manager".into()))
        }
    }

    async fn count_platform_configs_for_platform(
        &self,
        platform_str: String
    ) -> Result<usize, Error> {
        if let Some(am) = &self.auth_manager {
            let lock = am.lock().await;
            lock.count_platform_configs_for(platform_str.as_str()).await
        } else {
            Err(Error::Auth("No auth manager set in plugin manager".into()))
        }
    }

    // ---------- NEW METHODS: list_platform_configs / remove_platform_config ----------

    async fn list_platform_configs(
        &self,
        maybe_platform: Option<&str>
    ) -> Result<Vec<PlatformConfigData>, Error> {
        // Must access the `platform_config_repo` from AuthManager
        if let Some(am) = &self.auth_manager {
            let lock = am.lock().await;
            let pc_repo = &lock.platform_config_repo; // a dyn PlatformConfigRepository
            let rows = pc_repo.list_platform_configs(maybe_platform).await?;

            // Convert from DB model to your TUI struct
            let result: Vec<PlatformConfigData> = rows.into_iter().map(|r| {
                PlatformConfigData {
                    platform_config_id: r.platform_config_id,
                    platform: r.platform,
                    platform_label: r.platform_label,
                    client_id: r.client_id,
                    client_secret: r.client_secret,
                }
            }).collect();
            Ok(result)
        } else {
            Err(Error::Auth("No auth manager set in plugin manager".into()))
        }
    }

    async fn remove_platform_config(&self, platform_config_id: &str) -> Result<(), Error> {
        // Must access `platform_config_repo` again
        if let Some(am) = &self.auth_manager {
            let lock = am.lock().await;
            let pc_repo = &lock.platform_config_repo;
            pc_repo.delete_platform_config(platform_config_id).await?;
            Ok(())
        } else {
            Err(Error::Auth("No auth manager set in plugin manager".into()))
        }
    }
}