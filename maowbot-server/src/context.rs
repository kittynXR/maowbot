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
use tracing::{info, error, warn};
use rand::{thread_rng, Rng};
use keyring::Entry;
use base64;
use maowbot_common::models::cache::{CacheConfig, TrimPolicy};
use maowbot_common::traits::repository_traits::*;
use maowbot_core::auth::manager::AuthManager;
use maowbot_core::auth::user_manager::DefaultUserManager;
use maowbot_core::cache::message_cache::ChatCache;
use maowbot_core::repositories::postgres::analytics::PostgresAnalyticsRepository;
use maowbot_core::repositories::postgres::bot_config::PostgresBotConfigRepository;
use maowbot_core::repositories::postgres::command_usage::PostgresCommandUsageRepository;
use maowbot_core::repositories::postgres::commands::PostgresCommandRepository;
use maowbot_core::repositories::postgres::credentials::PostgresCredentialsRepository;
use maowbot_core::repositories::postgres::drip::DripRepository;
use maowbot_core::repositories::postgres::discord::PostgresDiscordRepository;
use maowbot_core::repositories::postgres::platform_config::PostgresPlatformConfigRepository;
use maowbot_core::repositories::postgres::platform_identity::PlatformIdentityRepository;
use maowbot_core::repositories::postgres::redeem_usage::PostgresRedeemUsageRepository;
use maowbot_core::repositories::postgres::redeems::PostgresRedeemRepository;
use maowbot_core::repositories::postgres::user::UserRepository;
use maowbot_core::repositories::postgres::user_analysis::PostgresUserAnalysisRepository;
use maowbot_osc::MaowOscManager;
use maowbot_osc::oscquery::OscQueryServer;
use maowbot_osc::robo::RoboControlSystem;
use maowbot_osc::vrchat::AvatarWatcher;

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
    pub ai_service: Option<Arc<maowbot_ai::plugins::ai_service::AiService>>,
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
        
        // Check if we should nuke the database and start fresh
        if args.nuke_database_and_start_fresh {
            info!("--nuke-database-and-start-fresh flag detected. Dropping all tables...");
            
            // Execute DROP commands for all tables
            sqlx::query(
                r#"
                DO $$ DECLARE
                    r RECORD;
                BEGIN
                    FOR r IN (SELECT tablename FROM pg_tables WHERE schemaname = 'public') LOOP
                        EXECUTE 'DROP TABLE IF EXISTS ' || quote_ident(r.tablename) || ' CASCADE';
                    END LOOP;
                END $$;
                "#
            )
            .execute(db.pool())
            .await?;
            
            info!("Database nuked successfully. Rebuilding from scratch...");
        }
        
        // Apply migrations (this will recreate tables if they were dropped)
        db.migrate().await?;

        // Possibly create an owner user if users table is empty
        maybe_create_owner_user(&db).await?;

        // 3) Build core repos
        let encryptor = Encryptor::new(&get_master_key()?)?;
        let creds_repo_arc = Arc::new(PostgresCredentialsRepository::new(db.pool().clone(), encryptor.clone()));
        let platform_config_repo = Arc::new(PostgresPlatformConfigRepository::new(db.pool().clone()));
        let bot_config_repo: Arc<dyn BotConfigRepository + Send + Sync> = Arc::new(
            PostgresBotConfigRepository::new(db.pool().clone())
        );
        let analytics_repo = Arc::new(PostgresAnalyticsRepository::new(db.pool().clone()));
        let user_analysis_repo = Arc::new(PostgresUserAnalysisRepository::new(db.pool().clone()));
        let user_repo_arc = Arc::new(UserRepository::new(db.pool().clone()));
        let drip_repo = Arc::new(DripRepository::new(db.pool().clone()));
        let discord_repo = Arc::new(PostgresDiscordRepository::new(db.pool().clone()));
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

        // Create a Discord repository
        let discord_repo = Arc::new(maowbot_core::repositories::postgres::discord::PostgresDiscordRepository::new(db.pool().clone()));
        
        // Platform manager
        let platform_manager = Arc::new(PlatformManager::new(
            user_service.clone(),
            event_bus.clone(),
            creds_repo_arc.clone(),
            discord_repo.clone(), // Pass Discord repository to PlatformManager
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
            creds_repo_arc.clone(),
        ));

        let eventsub_service = Arc::new(EventSubService::new(
            event_bus.clone(),
            redeem_service.clone(),
            user_service.clone(),
            platform_manager.clone(),
            bot_config_repo.clone(),
            discord_repo.clone(),
        ));

        // Create the AI repositories
        info!("🧪 Creating AI repositories...");
        let ai_provider_repo = Arc::new(maowbot_core::repositories::postgres::ai::PostgresAiProviderRepository::new(db.pool().clone()));
        let ai_credential_repo = Arc::new(maowbot_core::repositories::postgres::ai::PostgresAiCredentialRepository::new(db.pool().clone(), encryptor.clone()));
        let ai_model_repo = Arc::new(maowbot_core::repositories::postgres::ai::PostgresAiModelRepository::new(db.pool().clone()));
        let ai_trigger_repo = Arc::new(maowbot_core::repositories::postgres::ai::PostgresAiTriggerRepository::new(db.pool().clone()));
        let ai_memory_repo = Arc::new(maowbot_core::repositories::postgres::ai::PostgresAiMemoryRepository::new(db.pool().clone()));
        let ai_agent_repo = Arc::new(maowbot_core::repositories::postgres::ai::PostgresAiAgentRepository::new(db.pool().clone()));
        let ai_action_repo = Arc::new(maowbot_core::repositories::postgres::ai::PostgresAiActionRepository::new(db.pool().clone()));
        let ai_prompt_repo = Arc::new(maowbot_core::repositories::postgres::ai::PostgresAiSystemPromptRepository::new(db.pool().clone()));
        let ai_config_repo = Arc::new(maowbot_core::repositories::postgres::ai::PostgresAiConfigurationRepository::new(db.pool().clone(), encryptor.clone()));
        info!("🧪 AI repositories created successfully");

        // Create the AI service with repositories for full database integration
        info!("🧪 Initializing AI service with repositories...");
        let ai_service = match maowbot_ai::plugins::ai_service::AiService::with_repositories(
            user_repo_arc.clone(),
            creds_repo_arc.clone(),
            ai_provider_repo,
            ai_credential_repo,
            ai_model_repo,
            ai_trigger_repo,
            ai_memory_repo,
            ai_agent_repo,
            ai_action_repo,
            ai_prompt_repo,
            ai_config_repo
        ).await {
            Ok(service) => {
                info!("🧪 AI service initialized successfully with database repositories");
                
                // Configure with a default provider if environment variable exists
                if let Ok(api_key) = std::env::var("OPENAI_API_KEY") {
                    info!("🧪 Found OPENAI_API_KEY in environment, configuring default provider");
                    let mut options = std::collections::HashMap::new();
                    options.insert("system_prompt".to_string(), "You are MaowBot, a helpful AI assistant for Discord and Twitch users.".to_string());
                    
                    let config = maowbot_ai::models::ProviderConfig {
                        provider_type: "openai".to_string(),
                        api_key,
                        default_model: "gpt-4o".to_string(),
                        api_base: None,
                        options,
                    };
                    
                    match service.configure_provider(config).await {
                        Ok(_) => info!("🧪 Configured OpenAI provider from environment variable"),
                        Err(e) => error!("🧪 Failed to configure AI provider: {:?}", e),
                    }
                } else {
                    info!("🧪 No OPENAI_API_KEY found in environment, skipping default configuration");
                }
                
                // Print the trigger prefixes
                match service.get_trigger_prefixes().await {
                    Ok(prefixes) => info!("🧪 AI service trigger prefixes: {:?}", prefixes),
                    Err(e) => error!("🧪 Failed to get AI trigger prefixes: {:?}", e),
                }
                
                info!("🧪 Is AI service enabled? {}", service.is_enabled().await);
                Some(Arc::new(service))
            },
            Err(e) => {
                error!("🧪 Failed to initialize AI service with repositories: {:?}", e);
                None
            }
        };
        
        // Create a real AI API implementation instead of a stub
        let ai_api_impl = if let Some(ai_svc) = ai_service.clone() {
            info!("🧪 Creating AiApiImpl with real AI service");
            maowbot_core::plugins::manager::ai_api_impl::AiApiImpl::new(ai_svc.clone())
        } else {
            error!("🧪 AI service not available, creating STUB AiApiImpl");
            maowbot_core::plugins::manager::ai_api_impl::AiApiImpl::new_stub()
        };
        
        // 7) Plugin manager
        let mut plugin_manager = PluginManager::new(
            args.plugin_passphrase.clone(),
            user_repo_arc,
            drip_repo,
            discord_repo,
            analytics_repo,
            user_analysis_repo,
            platform_identity_repo,
            platform_manager.clone(),
            user_service,
            command_service.clone(),
            redeem_service.clone(),
            cmd_usage_repo,
            redeem_usage_repo,
            creds_repo_arc.clone(),
            Some(ai_api_impl.clone())
        );
        // Let plugin manager see the event bus
        plugin_manager.set_event_bus(event_bus.clone());
        plugin_manager.set_auth_manager(auth_manager_arc.clone());
        
        // Subscribe to event bus - critical for AI functionality!
        info!("Subscribing plugin manager to event bus");
        plugin_manager.subscribe_to_event_bus(event_bus.clone()).await;

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
        let mut osc_manager = MaowOscManager::new();

        // Set up the VRChat avatar watcher if VRChat directories are found
        if let Some(avatar_dir) = maowbot_osc::vrchat::get_vrchat_avatar_dir() {
            tracing::info!("Found VRChat avatar directory: {}", avatar_dir.display());
            let avatar_watcher = Arc::new(Mutex::new(maowbot_osc::vrchat::avatar_watcher::AvatarWatcher::new(avatar_dir)));
            osc_manager.set_vrchat_watcher(avatar_watcher);
        } else {
            tracing::warn!("VRChat avatar directory not found - avatar watcher disabled");
        }

        // After we're done with mutations, create the Arc
        let osc_manager_arc = Arc::new(osc_manager);

        // Create the new robo system:
        let robo_control = Arc::new(Mutex::new(RoboControlSystem::new()));

        plugin_manager.set_osc_manager(Arc::clone(&osc_manager_arc));

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
            osc_manager: osc_manager_arc.clone(),
            robo_control,
            oscquery_server: Arc::clone(&osc_manager_arc.oscquery_server),
            ai_service,
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

/// Gets a master key from the system keyring or generates a new one.
/// 
/// Uses the following strategy:
/// 1. Try to get/set the key from/to the system keyring (KDE KWallet, GNOME Keyring, or Windows/macOS native)
/// 2. If that fails, falls back to safely storing the key in a file with secure permissions
fn get_master_key() -> Result<[u8; 32], Error> {
    let service_name = "maowbot";
    let user_name = "master-key";

    // On Linux, log which desktop environment is running to help with debugging
    #[cfg(target_os = "linux")]
    {
        let session_type = std::env::var("XDG_SESSION_TYPE").unwrap_or_default();
        let desktop_env = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default();
        tracing::info!("Detected Linux session: {}, desktop environment: {}", session_type, desktop_env);
    }

    // Try the OS keyring first
    let entry_result = Entry::new(service_name, user_name);
    match entry_result {
        Ok(entry) => {
            match entry.get_password() {
                Ok(base64_key) => {
                    match decode_key(&base64_key) {
                        Ok(key) => {
                            tracing::info!("Retrieved existing master key from system keyring");
                            return Ok(key);
                        },
                        Err(e) => {
                            tracing::warn!("Found key in keyring but couldn't decode it: {}", e);
                            // Continue to re-generate key
                        }
                    }
                },
                Err(e) => {
                    tracing::info!("Couldn't retrieve key from keyring: {}", e);
                    // Continue to generate a new key
                }
            }

            // Generate a new key
            let mut new_key = [0u8; 32];
            thread_rng().fill(&mut new_key);
            let base64_key = base64::encode(new_key);
            
            // Try to save it to the keyring
            match entry.set_password(&base64_key) {
                Ok(_) => {
                    tracing::info!("Stored new master key in system keyring");
                    return Ok(new_key);
                },
                Err(e) => {
                    tracing::warn!("Failed to store key in system keyring: {}. Trying fallback storage...", e);
                    // Continue to fallback storage
                }
            }
        },
        Err(e) => {
            tracing::warn!("Couldn't create keyring entry: {}. Trying fallback storage...", e);
            // Continue to fallback storage
        }
    }

    // Fallback: Check for a securely stored file
    // This is a last resort if the OS keyring fails
    if let Some(key) = try_get_key_from_secure_file()? {
        return Ok(key);
    }

    // If we got here, we need to generate a new key and store it in the fallback
    let mut new_key = [0u8; 32];
    thread_rng().fill(&mut new_key);
    let base64_key = base64::encode(new_key);
    
    // Store in secure file
    if let Err(e) = store_key_in_secure_file(&base64_key) {
        tracing::warn!("Failed to store key in secure file: {}", e);
        tracing::warn!("WARNING: Using a temporary encryption key that will change on restart!");
        tracing::warn!("To fix this, please set up a compatible keyring service.");
    } else {
        tracing::info!("Stored new master key in secure file (fallback storage)");
    }
    
    Ok(new_key)
}

/// Decodes a base64 key into a 32-byte array
fn decode_key(base64_key: &str) -> Result<[u8; 32], Error> {
    tracing::debug!("Decoding base64 key of length: {}", base64_key.len());
    
    let key_bytes = base64::decode(base64_key)
        .map_err(|e| Error::Parse(format!("Failed to decode key: {:?}", e)))?;
    
    let key_len = key_bytes.len();
    tracing::debug!("Decoded to {} bytes", key_len);
    
    // Print first few bytes for debugging (safely)
    if !key_bytes.is_empty() {
        let preview = format!("{:02x}{:02x}{:02x}...", 
            key_bytes[0], 
            key_bytes.get(1).unwrap_or(&0), 
            key_bytes.get(2).unwrap_or(&0));
        tracing::debug!("Key starts with: {}", preview);
    }
    
    key_bytes.try_into()
        .map_err(|_| Error::Parse(format!("Key was not 32 bytes (got {} bytes)", key_len)))
}

/// Tries to get the key from a secure file
fn try_get_key_from_secure_file() -> Result<Option<[u8; 32]>, Error> {
    let key_file_path = get_secure_key_path()?;
    
    if !key_file_path.exists() {
        return Ok(None);
    }
    
    // Try to read the key file
    match std::fs::read_to_string(&key_file_path) {
        Ok(base64_key) => {
            match decode_key(&base64_key) {
                Ok(key) => {
                    tracing::info!("Retrieved master key from secure file: {}", key_file_path.display());
                    Ok(Some(key))
                },
                Err(e) => {
                    tracing::warn!("Found key file but couldn't decode it: {}", e);
                    Ok(None)
                }
            }
        },
        Err(e) => {
            tracing::warn!("Error reading key file: {}", e);
            Ok(None)
        }
    }
}

/// Stores the key in a secure file with restrictive permissions
fn store_key_in_secure_file(base64_key: &str) -> Result<(), Error> {
    let key_file_path = get_secure_key_path()?;
    
    // Ensure parent directory exists
    if let Some(parent) = key_file_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| Error::Io(e))?;
    }
    
    // Write the key to the file
    std::fs::write(&key_file_path, base64_key).map_err(|e| Error::Io(e))?;
    
    // Set restrictive permissions on Unix-like systems
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&key_file_path)
            .map_err(|e| Error::Io(e))?
            .permissions();
        // Only owner can read/write
        perms.set_mode(0o600);
        std::fs::set_permissions(&key_file_path, perms)
            .map_err(|e| Error::Io(e))?;
    }
    
    Ok(())
}

/// Gets the path to the secure key file
fn get_secure_key_path() -> Result<std::path::PathBuf, Error> {
    dirs::config_dir()
        .map(|dir| dir.join("maowbot").join("master.key"))
        .ok_or_else(|| Error::Keyring("Could not determine config directory".to_string()))
}
