//! plugins/manager/core.rs
//!
//! Contains the `PluginManager` struct and general logic not tied to BotApi sub-traits.

use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use async_trait::async_trait;
use futures_util::StreamExt;
use libloading::{Library, Symbol};
use tokio::sync::{mpsc::UnboundedSender, Mutex as AsyncMutex};
use tracing::{info, error};

use crate::Error;
use crate::eventbus::{BotEvent, EventBus};
use crate::plugins::plugin_connection::{
    PluginConnection, PluginConnectionInfo,
    PluginGrpcConnection, InProcessPluginConnection
};
use crate::plugins::types::{
    PluginType,
    PluginRecord,
    PluginStatesFile
};
use crate::repositories::postgres::user::{UserRepo, UserRepository};
use crate::auth::AuthManager;
use crate::eventbus::db_logger_handle::DbLoggerControl;
use crate::models::User;
use crate::platforms::manager::PlatformManager;
use crate::services::message_service::MessageService;
use crate::plugins::manager::plugin_api_impl::build_status_response;
use crate::repositories::{CommandUsageRepository, RedeemUsageRepository};
// or you can keep the function local
use crate::repositories::postgres::bot_config::BotConfigRepository;
use crate::repositories::postgres::credentials::CredentialsRepository;
use crate::repositories::postgres::platform_config::PlatformConfigRepository;
use crate::repositories::postgres::{PlatformIdentityRepository, PostgresAnalyticsRepository, PostgresUserAnalysisRepository};
use crate::services::{CommandService, RedeemService};
use crate::services::user_service::UserService;

/// The main manager that loads/stores plugins, spawns connections,
/// listens to inbound plugin messages, etc.
#[derive(Clone)]
pub struct PluginManager {
    /// The active plugin connections (both gRPC and in-process).
    pub plugins: Arc<AsyncMutex<Vec<Arc<dyn PluginConnection>>>>,

    /// The list of all known plugins (name + type + enabled).
    /// Serialized to/from JSON file.
    pub plugin_records: Arc<Mutex<Vec<PluginRecord>>>,

    /// Optional passphrase for validating inbound gRPC plugin connections.
    pub passphrase: Option<String>,

    /// The time we started (for uptime).
    pub start_time: Instant,

    pub(crate) db_logger_handle: Option<Arc<DbLoggerControl>>,
    /// The global event bus, if set.
    pub event_bus: Option<Arc<EventBus>>,

    /// Where we store plugin_records JSON.
    pub persist_path: PathBuf,

    /// If set, the auth manager for credential flows.
    pub auth_manager: Option<Arc<tokio::sync::Mutex<AuthManager>>>,

    /// A user repository so we can create/remove user rows, etc.
    pub user_repo: Arc<UserRepository>,

    /// The manager for platform logic (starting/stopping runtimes, etc.).
    pub platform_manager: Arc<PlatformManager>,

    pub analytics_repo: Arc<PostgresAnalyticsRepository>,
    pub user_analysis_repo: Arc<PostgresUserAnalysisRepository>,
    pub platform_identity_repo: Arc<PlatformIdentityRepository>,
    pub user_service: Arc<UserService>,

    pub command_service: Arc<CommandService>,
    pub redeem_service: Arc<RedeemService>,
    pub command_usage_repo: Arc<dyn CommandUsageRepository + Send + Sync>,
    pub redeem_usage_repo: Arc<dyn RedeemUsageRepository + Send + Sync>,
}

impl PluginManager {
    /// Constructs a new `PluginManager` with empty plugin lists and so on.
    pub fn new(
        passphrase: Option<String>,
        user_repo: Arc<UserRepository>,
        analytics_repo: Arc<PostgresAnalyticsRepository>,
        user_analysis_repo: Arc<PostgresUserAnalysisRepository>,
        platform_identity_repo: Arc<PlatformIdentityRepository>,
        platform_manager: Arc<PlatformManager>,
        user_service: Arc<UserService>,
        command_service: Arc<CommandService>,
        redeem_service: Arc<RedeemService>,
        cmd_usage_repo: Arc<dyn CommandUsageRepository + Send + Sync>,
        redeem_usage_repo: Arc<dyn RedeemUsageRepository + Send + Sync>,
    ) -> Self {
        let manager = Self {
            plugins: Arc::new(AsyncMutex::new(Vec::new())),
            plugin_records: Arc::new(Mutex::new(Vec::new())),
            passphrase,
            start_time: Instant::now(),
            db_logger_handle: None,
            event_bus: None,
            persist_path: PathBuf::from("plugs/plugins_state.json"),
            auth_manager: None,
            user_repo,
            platform_manager,

            analytics_repo,
            user_analysis_repo,
            platform_identity_repo,
            user_service,

            command_service,
            redeem_service,
            command_usage_repo: cmd_usage_repo,
            redeem_usage_repo,
        };
        manager.load_plugin_states();
        manager
    }

    /// Sets the global AuthManager if you want to support OAuth flows or credential logic.
    pub fn set_auth_manager(&mut self, am: Arc<tokio::sync::Mutex<AuthManager>>) {
        self.auth_manager = Some(am);
    }

    pub fn set_db_logger_handle(&mut self, handle: Arc<DbLoggerControl>) {
        self.db_logger_handle = Some(handle);
    }

    /// Attaches a shared `EventBus`.
    pub fn set_event_bus(&mut self, bus: Arc<EventBus>) {
        self.event_bus = Some(bus);
    }

    /// Subscribes the manager to events from the bus, so we can broadcast them to plugins if needed.
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
                                    // We can broadcast Tick to plugins if we want:
                                    use maowbot_proto::plugs::{
                                        PluginStreamResponse,
                                        plugin_stream_response::Payload as RespPayload,
                                        Tick
                                    };
                                    let tick_msg = PluginStreamResponse {
                                        payload: Some(RespPayload::Tick(Tick {})),
                                    };
                                    pm_clone.broadcast(tick_msg, None).await;
                                },
                                BotEvent::SystemMessage(msg) => {
                                    info!("(EventBus) SystemMessage => {}", msg);
                                }
                                _ => {}
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

    /// Called internally whenever a ChatMessage event arrives. We can broadcast to plugins if they have a chat capability.
    async fn handle_chat_event(&self, platform: &str, channel: &str, user: &str, text: &str) {
        use maowbot_proto::plugs::{
            PluginStreamResponse,
            plugin_stream_response::Payload as RespPayload,
            ChatMessage, PluginCapability
        };

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

    /// Broadcasts a single `PluginStreamResponse` to all loaded plugins that have `required_cap` (if specified).
    pub async fn broadcast(
        &self,
        response: maowbot_proto::plugs::PluginStreamResponse,
        required_cap: Option<maowbot_proto::plugs::PluginCapability>,
    ) {
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

    /// Loads the plugin states from disk. Called in `new()`.
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

    /// Saves the current plugin records to disk (JSON).
    pub fn save_plugin_states(&self) {
        let lock = self.plugin_records.lock().unwrap();
        let data = PluginStatesFile { plugins: lock.clone() };
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

    /// Returns a copy of the in-memory `plugin_records`.
    pub fn get_plugin_records(&self) -> Vec<PluginRecord> {
        self.plugin_records.lock().unwrap().clone()
    }

    /// Lists the currently connected plugin connections in memory.
    pub async fn list_connected_plugins(&self) -> Vec<PluginConnectionInfo> {
        let lock = self.plugins.lock().await;
        let mut out = Vec::new();
        for p in lock.iter() {
            out.push(p.info().await);
        }
        out
    }

    /// Insert or update a plugin_record in memory, then save to disk.
    pub fn upsert_plugin_record(&self, record: PluginRecord) {
        let mut lock = self.plugin_records.lock().unwrap();
        if let Some(existing) = lock.iter_mut().find(|r| r.name == record.name && r.plugin_type == record.plugin_type) {
            existing.enabled = record.enabled;
        } else {
            lock.push(record);
        }
        drop(lock);
        self.save_plugin_states();
    }

    /// Checks if the plugin is in `plugin_records` and if so, returns whether it’s enabled.
    /// If not found, returns false by default.
    pub fn is_plugin_enabled(&self, name: &str, plugin_type: &PluginType) -> bool {
        let lock = self.plugin_records.lock().unwrap();
        lock.iter()
            .find(|r| r.name == name && r.plugin_type == *plugin_type)
            .map(|r| r.enabled)
            .unwrap_or(false)
    }

    /// Called by the gRPC service to handle a new inbound plugin stream.
    pub async fn handle_new_grpc_stream(
        &self,
        mut inbound: tonic::Streaming<maowbot_proto::plugs::PluginStreamRequest>,
        sender: UnboundedSender<maowbot_proto::plugs::PluginStreamResponse>,
    ) {
        // Start off as disabled => we’ll enable after Hello passes.
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

    /// Processes an inbound message from a plugin (on the gRPC side).
    pub async fn on_inbound_message(
        &self,
        payload: maowbot_proto::plugs::plugin_stream_request::Payload,
        plugin: Arc<dyn PluginConnection>,
    ) {
        use maowbot_proto::plugs::{
            plugin_stream_request::Payload as ReqPayload,
            plugin_stream_response::Payload as RespPayload,
            Hello, AuthError, CapabilityResponse, ForceDisconnect,
            LogMessage, RequestCaps, Shutdown, SwitchScene, SendChat, Tick, RequestStatus
        };

        match payload {
            ReqPayload::Hello(Hello { plugin_name, passphrase }) => {
                // Passphrase check:
                if let Some(req_pass) = &self.passphrase {
                    if passphrase != *req_pass {
                        let err_resp = maowbot_proto::plugs::PluginStreamResponse {
                            payload: Some(RespPayload::AuthError(AuthError {
                                reason: "Invalid passphrase".into(),
                            })),
                        };
                        let _ = plugin.send(err_resp).await;
                        let _ = plugin.stop().await;
                        return;
                    }
                }
                // See if we have a record for this plugin:
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

                let welcome = maowbot_proto::plugs::PluginStreamResponse {
                    payload: Some(RespPayload::Welcome(maowbot_proto::plugs::WelcomeResponse {
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
                let status_msg = build_status_response(self).await;
                let _ = plugin.send(status_msg).await;
            }

            ReqPayload::RequestCaps(RequestCaps { requested }) => {
                let pi = plugin.info().await;
                if !pi.is_enabled {
                    return;
                }
                let (granted, denied) = self.evaluate_caps(&requested);
                plugin.set_capabilities(granted.clone()).await;
                let caps = maowbot_proto::plugs::PluginStreamResponse {
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
                let can_switch = pi.capabilities.contains(&maowbot_proto::plugs::PluginCapability::SceneManagement);
                if can_switch {
                    info!("Plugin '{}' switching scene => {}", pi.name, scene_name);
                    // Perform the real “scene switch” logic if any...
                } else {
                    let err = maowbot_proto::plugs::PluginStreamResponse {
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
                let can_send = pi.capabilities.contains(&maowbot_proto::plugs::PluginCapability::SendChat);
                if can_send {
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
                    let err = maowbot_proto::plugs::PluginStreamResponse {
                        payload: Some(RespPayload::AuthError(AuthError {
                            reason: "No SendChat capability".into(),
                        })),
                    };
                    let _ = plugin.send(err).await;
                }
            }
        }
    }

    fn evaluate_caps(
        &self,
        requested: &[i32]
    ) -> (
        Vec<maowbot_proto::plugs::PluginCapability>,
        Vec<maowbot_proto::plugs::PluginCapability>
    ) {
        use maowbot_proto::plugs::PluginCapability;
        let mut granted = Vec::new();
        let mut denied = Vec::new();

        for &cap_raw in requested {
            let cap = match cap_raw {
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

    /// Adds a plugin connection (in-process or gRPC) to our in-memory list.
    pub async fn add_plugin_connection(&self, plugin: Arc<dyn PluginConnection>) {
        let mut lock = self.plugins.lock().await;
        lock.push(plugin);
    }

    /// Removes a plugin connection from our in-memory list.
    pub async fn remove_plugin_connection(&self, plugin: &Arc<dyn PluginConnection>) {
        let info = plugin.info().await;
        let mut lock = self.plugins.lock().await;
        lock.retain(|p| {
            let pi = futures_lite::future::block_on(p.info());
            pi.name != info.name
        });
        info!("Removed plugin connection '{}'", info.name);
    }

    /// Discovers a plugin record from its .so/.dll path, then upserts it.
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

    /// Dynamically loads a single plugin library if it is “enabled”.
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

    /// Attempts to load an in-process plugin from the given path (dll/so).
    /// Also upserts the plugin record so we remember it next run.
    pub async fn load_in_process_plugin(&self, path: &str) -> Result<(), Error> {
        let rec = self.discover_dynamic_plugin(path);
        self.upsert_plugin_record(rec.clone());
        if rec.enabled {
            self.load_in_process_plugin_by_record(&rec).await?;
        }
        Ok(())
    }

    /// Tries to load all .dll/.so from a specified folder if they are “enabled”.
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
                                error!("Failed to load '{}' from {}: {:?}", rec.name, path_str, e);
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
}