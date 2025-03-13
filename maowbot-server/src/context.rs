//! maowbot-server/src/context.rs
//!
//! Defines the main "global" context (ServerContext) for the bot server.

use std::sync::Arc;
use tokio::sync::Mutex;
use maowbot_core::db::Database;
use maowbot_core::eventbus::EventBus;
use maowbot_core::crypto::Encryptor;
use maowbot_core::services::{message_service::MessageService, user_service::UserService, EventSubService};
use maowbot_core::services::twitch::{
    command_service::CommandService,
    redeem_service::RedeemService,
};
use maowbot_core::platforms::manager::PlatformManager;
use maowbot_core::plugins::manager::PluginManager;
use maowbot_core::Error;

use crate::Args;
use crate::portable_postgres::*;
use tracing::{info, error};
use rand::{thread_rng, Rng};
use keyring::Entry;
use base64;
use maowbot_common::models::cache::{CacheConfig, TrimPolicy};
use maowbot_common::traits::repository_traits::BotConfigRepository;
use maowbot_core::auth::manager::AuthManager;
use maowbot_core::auth::user_manager::DefaultUserManager;
use maowbot_core::cache::message_cache::ChatCache;
use maowbot_core::repositories::postgres::analytics::PostgresAnalyticsRepository;
use maowbot_core::repositories::postgres::bot_config::PostgresBotConfigRepository;
use maowbot_core::repositories::postgres::command_usage::PostgresCommandUsageRepository;
use maowbot_core::repositories::postgres::commands::PostgresCommandRepository;
use maowbot_core::repositories::postgres::credentials::PostgresCredentialsRepository;
use maowbot_core::repositories::postgres::drip::DripRepository;
use maowbot_core::repositories::postgres::platform_config::PostgresPlatformConfigRepository;
use maowbot_core::repositories::postgres::platform_identity::PlatformIdentityRepository;
use maowbot_core::repositories::postgres::redeem_usage::PostgresRedeemUsageRepository;
use maowbot_core::repositories::postgres::redeems::PostgresRedeemRepository;
use maowbot_core::repositories::postgres::user::UserRepository;
use maowbot_core::repositories::postgres::user_analysis::PostgresUserAnalysisRepository;
use maowbot_osc::MaowOscManager;
use maowbot_osc::oscquery::OscQueryServer;
use maowbot_osc::robo::RoboControlSystem;

/// The global server context (a bag of references to DB, event bus, plugin manager, etc.).
pub struct ServerContext {
    pub db: Database,
    pub event_bus: Arc<EventBus>,
    pub auth_manager: Arc<Mutex<AuthManager>>,
    pub message_service: Arc<MessageService>,
    pub platform_manager: Arc<PlatformManager>,
    pub plugin_manager: PluginManager,
    pub eventsub_service: Arc<EventSubService>,
    pub command_service: Arc<CommandService>,
    pub redeem_service: Arc<RedeemService>,

    /// The raw references in case you need them.
    pub creds_repo: Arc<PostgresCredentialsRepository>,
    pub bot_config_repo: Arc<dyn BotConfigRepository + Send + Sync>,

    pub osc_manager: Arc<MaowOscManager>,
    pub robo_control: Arc<Mutex<RoboControlSystem>>,
    pub oscquery_server: Arc<Mutex<OscQueryServer>>,
}

impl ServerContext {
    /// Creates and configures the entire context for "server" mode.
    pub async fn new(args: &Args) -> Result<Self, Error> {
        // 1) Start local Postgres (if needed)
        let pg_bin_dir = "./postgres/bin";
        let pg_data_dir = "./postgres/data";
        let port = 5432;

        if let Err(e) = kill_leftover_postgres_if_any(pg_bin_dir, pg_data_dir) {
            error!("Failed to handle leftover Postgres: {:?}", e);
        }

        ensure_db_initialized(pg_bin_dir, pg_data_dir)?;
        start_postgres(pg_bin_dir, pg_data_dir, port)?;
        create_database(pg_bin_dir, port, "maowbot")?;

        // 2) Connect to DB
        let db_url = &args.db_path;
        info!("Using Postgres DB URL: {}", db_url);
        let db = Database::new(db_url).await?;
        db.migrate().await?;

        // Possibly create an owner user if users table is empty
        maybe_create_owner_user(&db).await?;

        // 3) Build core repos
        let encryptor = Encryptor::new(&get_master_key()?)?;
        let creds_repo_arc = Arc::new(PostgresCredentialsRepository::new(db.pool().clone(), encryptor));
        let platform_config_repo = Arc::new(PostgresPlatformConfigRepository::new(db.pool().clone()));
        let bot_config_repo: Arc<dyn BotConfigRepository + Send + Sync> = Arc::new(
            PostgresBotConfigRepository::new(db.pool().clone())
        );
        let analytics_repo = Arc::new(PostgresAnalyticsRepository::new(db.pool().clone()));
        let user_analysis_repo = Arc::new(PostgresUserAnalysisRepository::new(db.pool().clone()));
        let user_repo_arc = Arc::new(UserRepository::new(db.pool().clone()));
        let drip_repo = Arc::new(DripRepository::new(db.pool().clone()));
        let platform_identity_repo = Arc::new(PlatformIdentityRepository::new(db.pool().clone()));
        let cmd_repo = Arc::new(PostgresCommandRepository::new(db.pool().clone()));
        let cmd_usage_repo = Arc::new(PostgresCommandUsageRepository::new(db.pool().clone()));
        let redeem_repo = Arc::new(PostgresRedeemRepository::new(db.pool().clone()));
        let redeem_usage_repo = Arc::new(PostgresRedeemUsageRepository::new(db.pool().clone()));

        // 4) Auth Manager
        let auth_manager = AuthManager::new(
            creds_repo_arc.clone(),
            platform_config_repo,
            bot_config_repo.clone(),
        );
        let auth_manager_arc = Arc::new(Mutex::new(auth_manager));

        // 5) Create EventBus
        let event_bus = Arc::new(EventBus::new());

        // 6) Construct user manager & services
        let default_user_mgr = DefaultUserManager::new(
            user_repo_arc.clone(),
            platform_identity_repo.clone(),
            user_analysis_repo.as_ref().clone(),
        );
        let user_manager_arc = Arc::new(default_user_mgr);
        let user_service = Arc::new(UserService::new(
            user_manager_arc.clone(),
            platform_identity_repo.clone(),
        ));

        // Chat cache
        let trim_policy = TrimPolicy {
            max_age_seconds: Some(24 * 3600),
            spam_score_cutoff: Some(5.0),
            max_total_messages: Some(10_000),
            max_messages_per_user: Some(200),
            min_quality_score: Some(0.2),
        };
        let cache_conf = CacheConfig { trim_policy };
        let chat_cache = Arc::new(Mutex::new(
            ChatCache::new(user_analysis_repo.as_ref().clone(), cache_conf)
        ));

        let command_service = Arc::new(CommandService::new(
            cmd_repo.clone(),
            cmd_usage_repo.clone(),
            creds_repo_arc.clone(),
            user_service.clone(),
            bot_config_repo.clone(),
        ));

        // Platform manager
        let platform_manager = Arc::new(PlatformManager::new(
            user_service.clone(),
            event_bus.clone(),
            creds_repo_arc.clone(),
        ));

        // Message service
        let message_service = Arc::new(MessageService::new(
            chat_cache,
            event_bus.clone(),
            user_manager_arc.clone(),
            user_service.clone(),
            command_service.clone(),
            platform_manager.clone(),
            creds_repo_arc.clone(),
        ));
        // Let the platform manager hold a reference to message_service
        platform_manager.set_message_service(message_service.clone());

        // Redeem service
        let redeem_service = Arc::new(RedeemService::new(
            redeem_repo.clone(),
            redeem_usage_repo.clone(),
            user_service.clone(),
            platform_manager.clone(),
        ));

        let eventsub_service = Arc::new(EventSubService::new(
            event_bus.clone(),
            redeem_service.clone(),
            user_service.clone(),
            platform_manager.clone(),
            bot_config_repo.clone(),
        ));

        // 7) Plugin manager
        let mut plugin_manager = PluginManager::new(
            args.plugin_passphrase.clone(),
            user_repo_arc,
            drip_repo,
            analytics_repo,
            user_analysis_repo,
            platform_identity_repo,
            platform_manager.clone(),
            user_service,
            command_service.clone(),
            redeem_service.clone(),
            cmd_usage_repo,
            redeem_usage_repo
        );
        // Let plugin manager see the event bus
        plugin_manager.subscribe_to_event_bus(event_bus.clone());
        plugin_manager.set_event_bus(event_bus.clone());
        plugin_manager.set_auth_manager(auth_manager_arc.clone());

        // Attempt to load optional in-process plugin
        if let Some(path) = &args.in_process_plugin {
            if let Err(e) = plugin_manager.load_in_process_plugin(path).await {
                error!("Failed to load in-process plugin from {}: {:?}", path, e);
            }
        }
        // Load all .so/dll in "plugs" folder
        if let Err(e) = plugin_manager.load_plugins_from_folder("plugs").await {
            error!("Failed to load plugins from 'plugs': {:?}", e);
        }

        // Create the new manager for OSC:
        let osc_manager = Arc::new(MaowOscManager::new());
        let oscquery_port = 8080;
        let oscquery_server = Arc::new(tokio::sync::Mutex::new(OscQueryServer::new(oscquery_port)));

        // Create the new robo system:
        let robo_control = Arc::new(Mutex::new(RoboControlSystem::new()));

        plugin_manager.set_osc_manager(Arc::clone(&osc_manager));

        Ok(ServerContext {
            db,
            event_bus,
            auth_manager: auth_manager_arc,
            message_service,
            platform_manager,
            plugin_manager,
            eventsub_service,
            command_service,
            redeem_service,
            creds_repo: creds_repo_arc,
            bot_config_repo: bot_config_repo,
            osc_manager,
            robo_control,
            oscquery_server,
        })
    }

    /// Shuts down the embedded Postgres instance if used.
    /// (Optional – you can call this after your server loop ends.)
    pub fn stop_postgres(&self) {
        let pg_bin_dir = "./postgres/bin";
        let pg_data_dir = "./postgres/data";
        let _ = crate::portable_postgres::stop_postgres(pg_bin_dir, pg_data_dir);
    }
}

/// If `users` table is empty, prompt once for an owner username.
async fn maybe_create_owner_user(db: &Database) -> Result<(), Error> {
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users")
        .fetch_one(db.pool())
        .await?;
    if count.0 == 0 {
        println!("No users found in DB. Let's create the owner account now.");
        println!("Enter the desired owner username:");
        let mut line = String::new();
        if std::io::stdin().read_line(&mut line).is_err() {
            return Err(Error::Io(std::io::Error::new(std::io::ErrorKind::Other, "Failed to read line")));
        }
        let owner_username = line.trim().to_string();
        if owner_username.is_empty() {
            return Err(Error::Auth("Owner username cannot be empty.".into()));
        }

        let user_id = uuid::Uuid::new_v4();
        let now = chrono::Utc::now();
        sqlx::query(
            r#"
            INSERT INTO users (user_id, global_username, created_at, last_seen, is_active)
            VALUES ($1, $2, $3, $4, true)
            "#
        )
            .bind(user_id)
            .bind(&owner_username)
            .bind(now)
            .bind(now)
            .execute(db.pool())
            .await?;

        sqlx::query(
            r#"
            INSERT INTO bot_config (config_key, config_value)
            VALUES ('owner_user_id', $1)
            ON CONFLICT (config_key, config_value) DO UPDATE
                SET config_value = EXCLUDED.config_value
            "#
        )
            .bind(user_id)
            .execute(db.pool())
            .await?;

        println!("Owner user '{}' created (user_id={}).", owner_username, user_id);
    }
    Ok(())
}

fn get_master_key() -> Result<[u8; 32], Error> {
    let service_name = "maowbot";
    let user_name = "master-key";
    let entry = Entry::new(service_name, user_name)?;

    match entry.get_password() {
        Ok(base64_key) => {
            let key_bytes = base64::decode(&base64_key)
                .map_err(|e| format!("Failed to decode key: {:?}", e))?;
            let key_32: [u8; 32] = key_bytes
                .try_into()
                .map_err(|_| "Stored key was not 32 bytes")?;
            println!("Retrieved existing master key from keyring.");
            Ok(key_32)
        },
        Err(_e) => {
            println!("No existing key found (or error retrieving key). Generating a new 32-byte key...");
            let mut new_key = [0u8; 32];
            thread_rng().fill(&mut new_key);
            let base64_key = base64::encode(new_key);
            if let Err(err) = entry.set_password(&base64_key) {
                println!("Failed to set key in keyring: {:?}", err);
            } else {
                println!("Stored new master key in keyring.");
            }
            Ok(new_key)
        }
    }
}
