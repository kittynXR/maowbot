//! maowbot-server/src/server.rs
//!
//! The main server logic: building the ServerContext and running the gRPC plugin service.

use maowbot_core::tasks::credential_refresh::refresh_expiring_tokens;
use std::sync::Arc;
use std::net::SocketAddr;
use std::time::Duration;
use tokio::time;
use tracing::{info, error, warn};
use tonic::transport::{Server, Identity, ServerTlsConfig};
use std::fs;
use std::path::Path;
use std::io::Write;
use rcgen::{generate_simple_self_signed};
use maowbot_core::Error;
use maowbot_core::eventbus::{BotEvent};
use maowbot_core::eventbus::db_logger::{spawn_db_logger_task};
use maowbot_core::eventbus::db_logger_handle::DbLoggerControl;
use maowbot_core::plugins::service_grpc::PluginServiceGrpc;
use maowbot_proto::plugs::plugin_service_server::PluginServiceServer;
use maowbot_core::plugins::manager::PluginManager;
use async_trait::async_trait;
use serde_json::Value;

use crate::Args;
use crate::context::ServerContext;
use crate::portable_postgres::*;
use maowbot_core::tasks::biweekly_maintenance::{
    spawn_biweekly_maintenance_task
};
use maowbot_core::tasks::credential_refresh::refresh_all_refreshable_credentials;
use maowbot_core::tasks::autostart::run_autostart;
use maowbot_core::tasks::redeem_sync;
use maowbot_core::tasks::discord_live_role;
use maowbot_tui::TuiModule;

pub async fn run_server(args: Args) -> Result<(), Error> {
    // Build the global context
    let ctx = ServerContext::new(&args).await?;

    // Start your OSC server on a free port:
    if let Err(e) = ctx.osc_manager.start_all().await {
        tracing::error!("Failed to start OSC/OSCQuery: {:?}", e);
    } else {
        tracing::info!("OSC and OSCQuery servers started successfully.");
    }

    // 1) Spawn DB logger
    let (db_logger_handle, _db_logger_control) = start_db_logger(&ctx);
    // 2) Spawn maintenance
    let _maintenance_task = spawn_biweekly_maintenance_task(
        ctx.db.clone(),
        maowbot_core::repositories::postgres::user_analysis::PostgresUserAnalysisRepository::new(ctx.db.pool().clone()),
        ctx.event_bus.clone()
    );

    redeem_sync::sync_channel_redeems(
        &ctx.redeem_service,
        &ctx.platform_manager,
        &ctx.message_service.user_service,
        &*ctx.bot_config_repo.clone(),
        false
    ).await?;

    // 3) Refresh credentials
    {
        let mut auth_lock = ctx.auth_manager.lock().await;
        if let Err(e) = refresh_all_refreshable_credentials(ctx.creds_repo.as_ref(), &mut *auth_lock).await {
            error!("Failed to refresh credentials on startup => {:?}", e);
        }
    }

    let creds_repo_clone = ctx.creds_repo.clone();
    let auth_manager_clone = ctx.auth_manager.clone();
    let _refresh_task = tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(30 * 60)); // Every 30 minutes
        loop {
            interval.tick().await;

            let mut auth_lock = auth_manager_clone.lock().await;
            match refresh_expiring_tokens(
                creds_repo_clone.as_ref(),
                &mut *auth_lock,
                60 // Refresh tokens expiring within 60 minutes
            ).await {
                Ok(_) => info!("Periodic token refresh completed"),
                Err(e) => error!("Periodic token refresh failed: {:?}", e),
            }
        }
    });

    // Create a proper BotApiWrapper that implements all BotApi traits including AiApi
    let bot_api = Arc::new(BotApiWrapper::new(ctx.plugin_manager.clone()));
    
    // 4) Autostart any configured accounts
    if let Err(e) = run_autostart(bot_api.as_ref(), bot_api.clone()).await {
        error!("Autostart error => {:?}", e);
    }
    
    // 4.5) Spawn Discord live role verification task after autostart
    // This task will check all users for streaming status and update roles at startup
    let _discord_live_role_startup_task = maowbot_core::tasks::discord_live_role::spawn_discord_live_role_startup_task(
        ctx.platform_manager.clone(),
        ctx.plugin_manager.discord_repo.clone()
    );
    
    // 4.6) Spawn periodic Discord live role check task
    // This task will regularly check and update streaming status
    let _discord_live_role_periodic_task = {
        // Find first active Discord account for periodic checks
        let discord_platform = {
            // Check active runtimes for Discord platforms
            let runtimes = ctx.platform_manager.active_runtimes.try_lock();
            if let Ok(guard) = runtimes {
                // Find first Discord instance directly from the runtime handle
                let discord_instance = guard.iter()
                    .find(|((platform, _), _)| platform == "discord")
                    .and_then(|((_platform, _account), handle)| handle.discord_instance.clone());
                
                if let Some(discord) = discord_instance {
                    Some(discord)
                } else {
                    error!("No Discord instances available for live role periodic task");
                    None
                }
            } else {
                error!("Failed to lock active runtimes for live role periodic task");
                None
            }
        };
        
        if let Some(discord) = discord_platform {
            // If we found an active Discord platform, spawn the periodic task
            maowbot_core::tasks::discord_live_role::spawn_discord_live_role_task(
                discord,
                ctx.plugin_manager.discord_repo.clone()
            )
        } else {
            // Otherwise, create a dummy task that just logs and exits
            tokio::spawn(async move {
                warn!("Discord live role periodic task not started - no Discord instances available");
            })
        }
    };
    
    // 5) If TUI was requested
    if args.tui {
        let tui_module = Arc::new(TuiModule::new(bot_api.clone(), ctx.event_bus.clone()).await);
        tui_module.spawn_tui_thread().await;
    }
    
    // Let active plugins see the BotApi
    {
        let lock = ctx.plugin_manager.plugins.lock().await;
        for p in lock.iter() {
            p.set_bot_api(bot_api.clone());
        }
    }

    let eventsub_svc_clone = ctx.eventsub_service.clone();
    tokio::spawn(async move {
        eventsub_svc_clone.start().await;
    });

    // 6) Start the gRPC server
    let identity = load_or_generate_certs()?;
    let tls_config = ServerTlsConfig::new().identity(identity);
    let addr: SocketAddr = args.server_addr.parse()?;
    info!("Starting Tonic gRPC server on {}", addr);

    let service_impl = PluginServiceGrpc {
        manager: ctx.plugin_manager.clone(),
    };
    let server_future = Server::builder()
        .tls_config(tls_config)?
        .add_service(PluginServiceServer::new(service_impl))
        .serve(addr);

    let event_bus = ctx.event_bus.clone();
    let srv_handle = tokio::spawn(async move {
        if let Err(e) = server_future.await {
            error!("gRPC server error: {:?}", e);
        }
    });

    // Ctrl-C => signal
    let eb_for_ctrlc = event_bus.clone();
    let _ctrlc_handle = tokio::spawn(async move {
        if let Err(e) = tokio::signal::ctrl_c().await {
            error!("Failed to listen for Ctrl‑C: {:?}", e);
        }
        info!("Ctrl‑C detected; shutting down event bus...");
        eb_for_ctrlc.shutdown();
    });

    // 7) Main loop => send Tick events until we see shutdown
    let mut shutdown_rx = event_bus.shutdown_rx.clone();
    loop {
        tokio::select! {
            _ = time::sleep(Duration::from_secs(10)) => {
                event_bus.publish(BotEvent::Tick).await;
            }
            Ok(_) = shutdown_rx.changed() => {
                if *shutdown_rx.borrow() {
                    info!("Shutdown signaled; exiting server loop.");
                    break;
                }
            }
        }
    }

    if let Err(e) = ctx.osc_manager.stop_all().await {
        error!("Failed to stop OSC/OSCQuery: {:?}", e);
    } else {
        info!("OSC and OSCQuery servers stopped successfully.");
    }

    // Force abort all OSC-related background tasks
    if let Some(rx) = ctx.osc_manager.take_osc_receiver().await {
        // Drop the receiver to close the channel
        drop(rx);
        info!("OSC receiver channel closed");
    }

    // Cleanup
    info!("Stopping gRPC server...");
    srv_handle.abort();
    info!("Stopping Postgres...");
    ctx.stop_postgres();
    info!("Server shutdown complete.");

    // Ensure DB logger is done
    db_logger_handle.abort();

    Ok(())
}

/// Spawns the DB-logger task, returns (JoinHandle, DbLoggerControl).
fn start_db_logger(ctx: &ServerContext) -> (tokio::task::JoinHandle<()>, DbLoggerControl) {
    let (jh, control) = spawn_db_logger_task(
        &ctx.event_bus,
        maowbot_core::repositories::postgres::analytics::PostgresAnalyticsRepository::new(ctx.db.pool().clone()),
        100,
        5,
    );
    (jh, control)
}

/// Load or generate self-signed TLS cert for gRPC.
fn load_or_generate_certs() -> Result<Identity, Error> {
    let cert_folder = "certs";
    let cert_path = format!("{}/server.crt", cert_folder);
    let key_path  = format!("{}/server.key", cert_folder);

    if Path::new(&cert_path).exists() && Path::new(&key_path).exists() {
        let cert_pem = fs::read(&cert_path)?;
        let key_pem  = fs::read(&key_path)?;
        return Ok(Identity::from_pem(cert_pem, key_pem));
    }

    let alt_names = vec!["localhost".to_string(), "127.0.0.1".to_string(), "0.0.0.0".to_string()];
    let certified = generate_simple_self_signed(alt_names)?;
    let cert_pem = certified.cert.pem();
    let key_pem = certified.key_pair.serialize_pem();

    fs::create_dir_all(cert_folder)?;
    fs::File::create(&cert_path)?.write_all(cert_pem.as_bytes())?;
    fs::File::create(&key_path)?.write_all(key_pem.as_bytes())?;

    Ok(Identity::from_pem(cert_pem, key_pem))
}

/// A wrapper for PluginManager that implements all the BotApi traits
/// including the AiApi trait
pub struct BotApiWrapper {
    /// The plugin manager we're wrapping
    plugin_manager: Arc<PluginManager>,
}

impl BotApiWrapper {
    pub fn new(plugin_manager: Arc<PluginManager>) -> Self {
        Self { plugin_manager }
    }
}

// Implement AiApi trait directly on the wrapper
#[async_trait]
impl maowbot_common::traits::api::AiApi for BotApiWrapper {
    /// Get the AI service for direct operations
    async fn get_ai_service(&self) -> Result<Option<std::sync::Arc<dyn std::any::Any + Send + Sync>>, maowbot_common::error::Error> {
        tracing::info!("🔍 BotApiWrapper: AiApi::get_ai_service called");
        match &self.plugin_manager.ai_api_impl {
            Some(ai) => {
                // First get the AiService from the implementation
                let service = ai.get_ai_service();
                // Then return it cast as Any
                Ok(service.map(|svc| svc as std::sync::Arc<dyn std::any::Any + Send + Sync>))
            },
            None => {
                tracing::warn!("🔍 BotApiWrapper: No AI API implementation available");
                Ok(None)
            },
        }
    }

    /// Generate a chat completion
    async fn generate_chat(&self, messages: Vec<Value>) -> Result<String, maowbot_common::error::Error> {
        match &self.plugin_manager.ai_api_impl {
            Some(ai) => ai.generate_chat(messages).await,
            None => Err(maowbot_common::error::Error::Internal("AI service not configured".to_string())),
        }
    }

    async fn generate_with_search(
        &self,
        messages: Vec<Value>,
    ) -> Result<Value, maowbot_common::error::Error> {
        match &self.plugin_manager.ai_api_impl {
            Some(ai) => ai.generate_with_search(messages).await,
            None => Err(maowbot_common::error::Error::Internal(
                "AI service not configured".to_string(),
            )),
        }
    }

    /// Generate a completion with function calling
    async fn generate_with_functions(&self, messages: Vec<Value>) -> Result<Value, maowbot_common::error::Error> {
        match &self.plugin_manager.ai_api_impl {
            Some(ai) => ai.generate_with_functions(messages).await,
            None => Err(maowbot_common::error::Error::Internal("AI service not configured".to_string())),
        }
    }
    
    /// Process a user message with context
    async fn process_user_message(&self, user_id: uuid::Uuid, message: &str) -> Result<String, maowbot_common::error::Error> {
        match &self.plugin_manager.ai_api_impl {
            Some(ai) => ai.process_user_message(user_id, message).await,
            None => Err(maowbot_common::error::Error::Internal("AI service not configured".to_string())),
        }
    }
    
    /// Register a new function
    async fn register_ai_function(&self, name: &str, description: &str) -> Result<(), maowbot_common::error::Error> {
        match &self.plugin_manager.ai_api_impl {
            Some(ai) => ai.register_ai_function(name, description).await,
            None => Err(maowbot_common::error::Error::Internal("AI service not configured".to_string())),
        }
    }
    
    /// Set the system prompt
    async fn set_system_prompt(&self, prompt: &str) -> Result<(), maowbot_common::error::Error> {
        match &self.plugin_manager.ai_api_impl {
            Some(ai) => ai.set_system_prompt(prompt).await,
            None => Err(maowbot_common::error::Error::Internal("AI service not configured".to_string())),
        }
    }
    
    /// Configure an AI provider with the given configuration
    async fn configure_ai_provider(&self, config: Value) -> Result<(), maowbot_common::error::Error> {
        match &self.plugin_manager.ai_api_impl {
            Some(ai) => ai.configure_ai_provider(config).await,
            None => Err(maowbot_common::error::Error::Internal("AI service not configured".to_string())),
        }
    }
}

// Forward all other BotApi trait calls to the plugin_manager

// PluginApi forwarding
#[async_trait]
impl maowbot_common::traits::api::PluginApi for BotApiWrapper {
    async fn list_plugins(&self) -> Vec<String> {
        self.plugin_manager.list_plugins().await
    }
    
    async fn status(&self) -> maowbot_common::models::plugin::StatusData {
        self.plugin_manager.status().await
    }
    
    async fn shutdown(&self) {
        self.plugin_manager.shutdown().await
    }
    
    async fn toggle_plugin(&self, plugin_name: &str, enable: bool) -> Result<(), maowbot_common::error::Error> {
        self.plugin_manager.toggle_plugin(plugin_name, enable).await
    }
    
    async fn remove_plugin(&self, plugin_name: &str) -> Result<(), maowbot_common::error::Error> {
        self.plugin_manager.remove_plugin(plugin_name).await
    }
    
    async fn subscribe_chat_events(&self, buffer_size: Option<usize>) -> tokio::sync::mpsc::Receiver<maowbot_common::models::analytics::BotEvent> {
        self.plugin_manager.subscribe_chat_events(buffer_size).await
    }
    
    async fn list_config(&self) -> Result<Vec<(String, String)>, maowbot_common::error::Error> {
        self.plugin_manager.list_config().await
    }
}

// UserApi forwarding
#[async_trait]
impl maowbot_common::traits::api::UserApi for BotApiWrapper {
    async fn create_user(&self, new_user_id: uuid::Uuid, display_name: &str) -> Result<(), maowbot_common::error::Error> {
        self.plugin_manager.create_user(new_user_id, display_name).await
    }
    
    async fn remove_user(&self, user_id: uuid::Uuid) -> Result<(), maowbot_common::error::Error> {
        self.plugin_manager.remove_user(user_id).await
    }
    
    async fn get_user(&self, user_id: uuid::Uuid) -> Result<Option<maowbot_common::models::user::User>, maowbot_common::error::Error> {
        self.plugin_manager.get_user(user_id).await
    }
    
    async fn update_user_active(&self, user_id: uuid::Uuid, is_active: bool) -> Result<(), maowbot_common::error::Error> {
        self.plugin_manager.update_user_active(user_id, is_active).await
    }
    
    async fn search_users(&self, query: &str) -> Result<Vec<maowbot_common::models::user::User>, maowbot_common::error::Error> {
        self.plugin_manager.search_users(query).await
    }
    
    async fn find_user_by_name(&self, name: &str) -> Result<maowbot_common::models::user::User, maowbot_common::error::Error> {
        self.plugin_manager.find_user_by_name(name).await
    }
    
    async fn get_user_chat_messages(
        &self,
        user_id: uuid::Uuid,
        limit: i64,
        offset: i64,
        maybe_platform: Option<String>,
        maybe_channel: Option<String>,
        maybe_search: Option<String>,
    ) -> Result<Vec<maowbot_common::models::analytics::ChatMessage>, maowbot_common::error::Error> {
        self.plugin_manager.get_user_chat_messages(user_id, limit, offset, maybe_platform, maybe_channel, maybe_search).await
    }
    
    async fn append_moderator_note(&self, user_id: uuid::Uuid, note_text: &str) -> Result<(), maowbot_common::error::Error> {
        self.plugin_manager.append_moderator_note(user_id, note_text).await
    }
    
    async fn get_platform_identities_for_user(&self, user_id: uuid::Uuid) -> Result<Vec<maowbot_common::models::platform::PlatformIdentity>, maowbot_common::error::Error> {
        self.plugin_manager.get_platform_identities_for_user(user_id).await
    }
    
    async fn get_user_analysis(&self, user_id: uuid::Uuid) -> Result<Option<maowbot_common::models::UserAnalysis>, maowbot_common::error::Error> {
        self.plugin_manager.get_user_analysis(user_id).await
    }
    
    async fn merge_users(
        &self,
        user1_id: uuid::Uuid,
        user2_id: uuid::Uuid,
        new_global_name: Option<&str>
    ) -> Result<(), maowbot_common::error::Error> {
        self.plugin_manager.merge_users(user1_id, user2_id, new_global_name).await
    }
    
    async fn add_role_to_user_identity(&self, user_id: uuid::Uuid, platform: &str, role: &str) -> Result<(), maowbot_common::error::Error> {
        self.plugin_manager.add_role_to_user_identity(user_id, platform, role).await
    }
    
    async fn remove_role_from_user_identity(&self, user_id: uuid::Uuid, platform: &str, role: &str) -> Result<(), maowbot_common::error::Error> {
        self.plugin_manager.remove_role_from_user_identity(user_id, platform, role).await
    }
}

// CredentialsApi
#[async_trait]
impl maowbot_common::traits::api::CredentialsApi for BotApiWrapper {
    async fn begin_auth_flow(&self, platform: maowbot_common::models::auth::Platform, is_bot: bool) -> Result<String, maowbot_common::error::Error> {
        self.plugin_manager.begin_auth_flow(platform, is_bot).await
    }
    
    async fn complete_auth_flow(
        &self,
        platform: maowbot_common::models::auth::Platform,
        code: String
    ) -> Result<maowbot_common::models::platform::PlatformCredential, maowbot_common::error::Error> {
        self.plugin_manager.complete_auth_flow(platform, code).await
    }
    
    async fn complete_auth_flow_for_user(
        &self,
        platform: maowbot_common::models::auth::Platform,
        code: String,
        user_id: uuid::Uuid
    ) -> Result<maowbot_common::models::platform::PlatformCredential, maowbot_common::error::Error> {
        self.plugin_manager.complete_auth_flow_for_user(platform, code, user_id).await
    }
    
    async fn complete_auth_flow_for_user_multi(
        &self,
        platform: maowbot_common::models::auth::Platform,
        user_id: uuid::Uuid,
        keys: std::collections::HashMap<String, String>,
    ) -> Result<maowbot_common::models::platform::PlatformCredential, maowbot_common::error::Error> {
        self.plugin_manager.complete_auth_flow_for_user_multi(platform, user_id, keys).await
    }
    
    async fn complete_auth_flow_for_user_2fa(
        &self,
        platform: maowbot_common::models::auth::Platform,
        code: String,
        user_id: uuid::Uuid
    ) -> Result<maowbot_common::models::platform::PlatformCredential, maowbot_common::error::Error> {
        self.plugin_manager.complete_auth_flow_for_user_2fa(platform, code, user_id).await
    }
    
    async fn revoke_credentials(
        &self,
        platform: maowbot_common::models::auth::Platform,
        user_id: String
    ) -> Result<(), maowbot_common::error::Error> {
        self.plugin_manager.revoke_credentials(platform, user_id).await
    }
    
    async fn refresh_credentials(
        &self,
        platform: maowbot_common::models::auth::Platform,
        user_id: String
    ) -> Result<maowbot_common::models::platform::PlatformCredential, maowbot_common::error::Error> {
        self.plugin_manager.refresh_credentials(platform, user_id).await
    }
    
    async fn list_credentials(
        &self,
        maybe_platform: Option<maowbot_common::models::auth::Platform>
    ) -> Result<Vec<maowbot_common::models::platform::PlatformCredential>, maowbot_common::error::Error> {
        self.plugin_manager.list_credentials(maybe_platform).await
    }
    
    async fn store_credential(&self, cred: maowbot_common::models::platform::PlatformCredential) -> Result<(), maowbot_common::error::Error> {
        self.plugin_manager.store_credential(cred).await
    }
}

// PlatformApi
#[async_trait]
impl maowbot_common::traits::api::PlatformApi for BotApiWrapper {
    async fn create_platform_config(
        &self,
        platform: maowbot_common::models::auth::Platform,
        client_id: String,
        client_secret: Option<String>
    ) -> Result<(), maowbot_common::error::Error> {
        self.plugin_manager.create_platform_config(platform, client_id, client_secret).await
    }

    async fn count_platform_configs_for_platform(
        &self,
        platform_str: String
    ) -> Result<usize, maowbot_common::error::Error> {
        self.plugin_manager.count_platform_configs_for_platform(platform_str).await
    }

    async fn list_platform_configs(
        &self,
        maybe_platform: Option<&str>
    ) -> Result<Vec<maowbot_common::models::platform::PlatformConfigData>, maowbot_common::error::Error> {
        self.plugin_manager.list_platform_configs(maybe_platform).await
    }

    async fn remove_platform_config(
        &self,
        platform_config_id: &str
    ) -> Result<(), maowbot_common::error::Error> {
        self.plugin_manager.remove_platform_config(platform_config_id).await
    }

    async fn start_platform_runtime(&self, platform: &str, account_name: &str) -> Result<(), maowbot_common::error::Error> {
        self.plugin_manager.start_platform_runtime(platform, account_name).await
    }
    
    async fn stop_platform_runtime(&self, platform: &str, account_name: &str) -> Result<(), maowbot_common::error::Error> {
        self.plugin_manager.stop_platform_runtime(platform, account_name).await
    }
}

// TwitchApi
#[async_trait]
impl maowbot_common::traits::api::TwitchApi for BotApiWrapper {
    async fn join_twitch_irc_channel(&self, account_name: &str, channel: &str) -> Result<(), maowbot_common::error::Error> {
        self.plugin_manager.join_twitch_irc_channel(account_name, channel).await
    }
    
    async fn part_twitch_irc_channel(&self, account_name: &str, channel: &str) -> Result<(), maowbot_common::error::Error> {
        self.plugin_manager.part_twitch_irc_channel(account_name, channel).await
    }
    
    async fn send_twitch_irc_message(&self, account_name: &str, channel: &str, text: &str) -> Result<(), maowbot_common::error::Error> {
        self.plugin_manager.send_twitch_irc_message(account_name, channel, text).await
    }

    async fn timeout_twitch_user(&self, account_name: &str, channel: &str, target_user: &str, seconds: u32, reason: Option<&str>) -> Result<(), Error> {
        self.plugin_manager.timeout_twitch_user(account_name, channel, target_user, seconds, reason).await
    }
}

// VrchatApi
#[async_trait]
impl maowbot_common::traits::api::VrchatApi for BotApiWrapper {
    async fn vrchat_get_current_world(&self, account_name: &str) -> Result<maowbot_common::models::vrchat::VRChatWorldBasic, maowbot_common::error::Error> {
        self.plugin_manager.vrchat_get_current_world(account_name).await
    }
    
    async fn vrchat_get_current_avatar(&self, account_name: &str) -> Result<maowbot_common::models::vrchat::VRChatAvatarBasic, maowbot_common::error::Error> {
        self.plugin_manager.vrchat_get_current_avatar(account_name).await
    }
    
    async fn vrchat_change_avatar(&self, account_name: &str, new_avatar_id: &str) -> Result<(), maowbot_common::error::Error> {
        self.plugin_manager.vrchat_change_avatar(account_name, new_avatar_id).await
    }
    
    async fn vrchat_get_current_instance(&self, account_name: &str) -> Result<maowbot_common::models::vrchat::VRChatInstanceBasic, maowbot_common::error::Error> {
        self.plugin_manager.vrchat_get_current_instance(account_name).await
    }
}

// CommandApi
#[async_trait]
impl maowbot_common::traits::api::CommandApi for BotApiWrapper {
    async fn list_commands(&self, platform: &str) -> Result<Vec<maowbot_common::models::Command>, maowbot_common::error::Error> {
        self.plugin_manager.list_commands(platform).await
    }
    
    async fn create_command(&self, platform: &str, command_name: &str, min_role: &str) -> Result<maowbot_common::models::Command, maowbot_common::error::Error> {
        self.plugin_manager.create_command(platform, command_name, min_role).await
    }
    
    async fn set_command_active(&self, command_id: uuid::Uuid, is_active: bool) -> Result<(), maowbot_common::error::Error> {
        self.plugin_manager.set_command_active(command_id, is_active).await
    }
    
    async fn update_command_role(&self, command_id: uuid::Uuid, new_role: &str) -> Result<(), maowbot_common::error::Error> {
        self.plugin_manager.update_command_role(command_id, new_role).await
    }
    
    async fn delete_command(&self, command_id: uuid::Uuid) -> Result<(), maowbot_common::error::Error> {
        self.plugin_manager.delete_command(command_id).await
    }
    
    async fn get_usage_for_command(&self, command_id: uuid::Uuid, limit: i64) -> Result<Vec<maowbot_common::models::CommandUsage>, maowbot_common::error::Error> {
        self.plugin_manager.get_usage_for_command(command_id, limit).await
    }
    
    async fn get_usage_for_user(&self, user_id: uuid::Uuid, limit: i64) -> Result<Vec<maowbot_common::models::CommandUsage>, maowbot_common::error::Error> {
        self.plugin_manager.get_usage_for_user(user_id, limit).await
    }
    
    async fn update_command(&self, updated_cmd: &maowbot_common::models::Command) -> Result<(), maowbot_common::error::Error> {
        self.plugin_manager.update_command(updated_cmd).await
    }
}

// RedeemApi
#[async_trait]
impl maowbot_common::traits::api::RedeemApi for BotApiWrapper {
    async fn list_redeems(&self, platform: &str) -> Result<Vec<maowbot_common::models::Redeem>, maowbot_common::error::Error> {
        self.plugin_manager.list_redeems(platform).await
    }
    
    async fn create_redeem(&self, platform: &str, reward_id: &str, reward_name: &str, cost: i32, dynamic: bool)
                           -> Result<maowbot_common::models::Redeem, maowbot_common::error::Error> {
        self.plugin_manager.create_redeem(platform, reward_id, reward_name, cost, dynamic).await
    }
    
    async fn set_redeem_active(&self, redeem_id: uuid::Uuid, is_active: bool) -> Result<(), maowbot_common::error::Error> {
        self.plugin_manager.set_redeem_active(redeem_id, is_active).await
    }
    
    async fn update_redeem_cost(&self, redeem_id: uuid::Uuid, new_cost: i32) -> Result<(), maowbot_common::error::Error> {
        self.plugin_manager.update_redeem_cost(redeem_id, new_cost).await
    }
    
    async fn delete_redeem(&self, redeem_id: uuid::Uuid) -> Result<(), maowbot_common::error::Error> {
        self.plugin_manager.delete_redeem(redeem_id).await
    }
    
    async fn get_usage_for_redeem(&self, redeem_id: uuid::Uuid, limit: i64) -> Result<Vec<maowbot_common::models::RedeemUsage>, maowbot_common::error::Error> {
        self.plugin_manager.get_usage_for_redeem(redeem_id, limit).await
    }
    
    async fn get_usage_for_user(&self, user_id: uuid::Uuid, limit: i64) -> Result<Vec<maowbot_common::models::RedeemUsage>, maowbot_common::error::Error> {
        self.plugin_manager.get_usage_for_user(user_id, limit).await
    }
    
    async fn update_redeem(&self, redeem: &maowbot_common::models::Redeem) -> Result<(), maowbot_common::error::Error> {
        self.plugin_manager.update_redeem(redeem).await
    }
    
    async fn sync_redeems(&self) -> Result<(), maowbot_common::error::Error> {
        self.plugin_manager.sync_redeems().await
    }
}

// OscApi
#[async_trait]
impl maowbot_common::traits::api::OscApi for BotApiWrapper {
    async fn osc_start(&self) -> Result<(), maowbot_common::error::Error> {
        self.plugin_manager.osc_start().await
    }
    
    async fn osc_stop(&self) -> Result<(), maowbot_common::error::Error> {
        self.plugin_manager.osc_stop().await
    }
    
    async fn osc_status(&self) -> Result<maowbot_common::models::osc::OscStatus, maowbot_common::error::Error> {
        self.plugin_manager.osc_status().await
    }
    
    async fn osc_chatbox(&self, message: &str) -> Result<(), maowbot_common::error::Error> {
        self.plugin_manager.osc_chatbox(message).await
    }
    
    async fn osc_discover_peers(&self) -> Result<Vec<String>, maowbot_common::error::Error> {
        self.plugin_manager.osc_discover_peers().await
    }
    
    async fn osc_take_raw_receiver(&self) -> Result<Option<tokio::sync::mpsc::UnboundedReceiver<rosc::OscPacket>>, maowbot_common::error::Error> {
        self.plugin_manager.osc_take_raw_receiver().await
    }
}

// DripApi
#[async_trait]
impl maowbot_common::traits::api::DripApi for BotApiWrapper {
    async fn drip_show_settable(&self) -> Result<String, maowbot_common::error::Error> {
        self.plugin_manager.drip_show_settable().await
    }
    
    async fn drip_set_ignore_prefix(&self, prefix: &str) -> Result<String, maowbot_common::error::Error> {
        self.plugin_manager.drip_set_ignore_prefix(prefix).await
    }
    
    async fn drip_set_strip_prefix(&self, prefix: &str) -> Result<String, maowbot_common::error::Error> {
        self.plugin_manager.drip_set_strip_prefix(prefix).await
    }
    
    async fn drip_set_avatar_name(&self, new_name: &str) -> Result<String, maowbot_common::error::Error> {
        self.plugin_manager.drip_set_avatar_name(new_name).await
    }
    
    async fn drip_list_avatars(&self) -> Result<Vec<maowbot_common::models::drip::DripAvatarSummary>, maowbot_common::error::Error> {
        self.plugin_manager.drip_list_avatars().await
    }
    
    async fn drip_fit_new(&self, fit_name: &str) -> Result<String, maowbot_common::error::Error> {
        self.plugin_manager.drip_fit_new(fit_name).await
    }
    
    async fn drip_fit_add_param(&self, fit_name: &str, param_name: &str, param_value: &str) -> Result<String, maowbot_common::error::Error> {
        self.plugin_manager.drip_fit_add_param(fit_name, param_name, param_value).await
    }
    
    async fn drip_fit_del_param(&self, fit_name: &str, param_name: &str, param_value: &str) -> Result<String, maowbot_common::error::Error> {
        self.plugin_manager.drip_fit_del_param(fit_name, param_name, param_value).await
    }
    
    async fn drip_fit_wear(&self, fit_name: &str) -> Result<String, maowbot_common::error::Error> {
        self.plugin_manager.drip_fit_wear(fit_name).await
    }
    
    async fn drip_props_add(&self, prop_name: &str, param_name: &str, param_value: &str) -> Result<String, maowbot_common::error::Error> {
        self.plugin_manager.drip_props_add(prop_name, param_name, param_value).await
    }
    
    async fn drip_props_del(&self, prop_name: &str, param_name: &str, param_value: &str) -> Result<String, maowbot_common::error::Error> {
        self.plugin_manager.drip_props_del(prop_name, param_name, param_value).await
    }
    
    async fn drip_props_timer(&self, prop_name: &str, timer_data: &str) -> Result<String, maowbot_common::error::Error> {
        self.plugin_manager.drip_props_timer(prop_name, timer_data).await
    }
}

// BotConfigApi
#[async_trait]
impl maowbot_common::traits::api::BotConfigApi for BotApiWrapper {
    async fn list_all_config(&self) -> Result<Vec<(String, String)>, maowbot_common::error::Error> {
        self.plugin_manager.list_all_config().await
    }
    
    async fn get_bot_config_value(&self, config_key: &str) -> Result<Option<String>, maowbot_common::error::Error> {
        self.plugin_manager.get_bot_config_value(config_key).await
    }
    
    async fn set_bot_config_value(&self, config_key: &str, config_value: &str) -> Result<(), maowbot_common::error::Error> {
        self.plugin_manager.set_bot_config_value(config_key, config_value).await
    }
    
    async fn delete_bot_config_key(&self, config_key: &str) -> Result<(), maowbot_common::error::Error> {
        self.plugin_manager.delete_bot_config_key(config_key).await
    }

    async fn set_config_kv_meta(
        &self,
        config_key: &str,
        config_value: &str,
        config_meta: Option<serde_json::Value>,
    ) -> Result<(), maowbot_common::error::Error> {
        self.plugin_manager.set_config_kv_meta(config_key, config_value, config_meta).await
    }

    async fn get_config_kv_meta(
        &self,
        config_key: &str,
        config_value: &str
    ) -> Result<Option<(String, Option<serde_json::Value>)>, maowbot_common::error::Error> {
        self.plugin_manager.get_config_kv_meta(config_key, config_value).await
    }

    async fn delete_config_kv(&self, config_key: &str, config_value: &str) -> Result<(), maowbot_common::error::Error> {
        self.plugin_manager.delete_config_kv(config_key, config_value).await
    }
}

// DiscordApi
#[async_trait]
impl maowbot_common::traits::api::DiscordApi for BotApiWrapper {
    async fn list_discord_guilds(&self, account_name: &str) -> Result<Vec<maowbot_common::models::discord::DiscordGuildRecord>, maowbot_common::error::Error> {
        self.plugin_manager.list_discord_guilds(account_name).await
    }
    
    async fn list_discord_channels(&self, account_name: &str, guild_id: &str) -> Result<Vec<maowbot_common::models::discord::DiscordChannelRecord>, maowbot_common::error::Error> {
        self.plugin_manager.list_discord_channels(account_name, guild_id).await
    }
    
    async fn send_discord_message(&self, account_name: &str, server_id: &str, channel_id: &str, text: &str) -> Result<(), maowbot_common::error::Error> {
        self.plugin_manager.send_discord_message(account_name, server_id, channel_id, text).await
    }
    
    async fn send_discord_embed(
        &self,
        account_name: &str,
        server_id: &str,
        channel_id: &str,
        embed: &maowbot_common::models::discord::DiscordEmbed,
        content: Option<&str>
    ) -> Result<(), maowbot_common::error::Error> {
        self.plugin_manager.send_discord_embed(account_name, server_id, channel_id, embed, content).await
    }
    
    async fn list_discord_event_configs(&self) -> Result<Vec<maowbot_common::models::discord::DiscordEventConfigRecord>, maowbot_common::error::Error> {
        self.plugin_manager.list_discord_event_configs().await
    }
    
    async fn add_discord_event_config(
        &self,
        event_name: &str,
        guild_id: &str,
        channel_id: &str,
        maybe_credential_id: Option<uuid::Uuid>
    ) -> Result<(), maowbot_common::error::Error> {
        self.plugin_manager.add_discord_event_config(event_name, guild_id, channel_id, maybe_credential_id).await
    }

    async fn remove_discord_event_config(&self, event_name: &str, guild_id: &str, channel_id: &str, maybe_credential_id: Option<uuid::Uuid>) -> Result<(), maowbot_common::error::Error> {
        self.plugin_manager.remove_discord_event_config(event_name, guild_id, channel_id, maybe_credential_id).await
    }

    async fn upsert_discord_account(&self, account_name: &str, credential_id: Option<uuid::Uuid>, discord_id: Option<&str>) -> Result<(), maowbot_common::error::Error> {
        self.plugin_manager.upsert_discord_account(account_name, credential_id, discord_id).await
    }

    async fn add_discord_event_role(&self, event_name: &str, role_id: &str) -> Result<(), maowbot_common::error::Error> {
        self.plugin_manager.add_discord_event_role(event_name, role_id).await
    }
    
    async fn remove_discord_event_role(&self, event_name: &str, role_id: &str) -> Result<(), maowbot_common::error::Error> {
        self.plugin_manager.remove_discord_event_role(event_name, role_id).await
    }
    
    async fn list_discord_roles(&self, account_name: &str, guild_id: &str) -> Result<Vec<(String, String)>, maowbot_common::error::Error> {
        self.plugin_manager.list_discord_roles(account_name, guild_id).await
    }
    
    // New Discord Live Role methods
    async fn set_discord_live_role(&self, guild_id: &str, role_id: &str) -> Result<(), maowbot_common::error::Error> {
        self.plugin_manager.set_discord_live_role(guild_id, role_id).await
    }
    
    async fn get_discord_live_role(&self, guild_id: &str) -> Result<Option<maowbot_common::models::discord::DiscordLiveRoleRecord>, maowbot_common::error::Error> {
        self.plugin_manager.get_discord_live_role(guild_id).await
    }
    
    async fn delete_discord_live_role(&self, guild_id: &str) -> Result<(), maowbot_common::error::Error> {
        self.plugin_manager.delete_discord_live_role(guild_id).await
    }
    
    async fn list_discord_live_roles(&self) -> Result<Vec<maowbot_common::models::discord::DiscordLiveRoleRecord>, maowbot_common::error::Error> {
        self.plugin_manager.list_discord_live_roles().await
    }
    
    async fn add_role_to_discord_user(&self, account_name: &str, guild_id: &str, user_id: &str, role_id: &str) -> Result<(), maowbot_common::error::Error> {
        self.plugin_manager.add_role_to_discord_user(account_name, guild_id, user_id, role_id).await
    }
    
    async fn remove_role_from_discord_user(&self, account_name: &str, guild_id: &str, user_id: &str, role_id: &str) -> Result<(), maowbot_common::error::Error> {
        self.plugin_manager.remove_role_from_discord_user(account_name, guild_id, user_id, role_id).await
    }
}
