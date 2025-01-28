// src/main.rs

use clap::Parser;
use std::time::Duration;
use std::net::SocketAddr;
use tokio::sync::Mutex;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tracing::{error, info};
use tracing_subscriber::FmtSubscriber;
use std::sync::Arc;

//
// Import your crate modules:
//
use maowbot::Database;
use maowbot::plugins::manager::PluginManager;
use maowbot::plugins::protocol::{BotToPlugin, PluginToBot};
// Removed the unused credential_refresh import
use maowbot::auth::{AuthManager, DefaultUserManager, StubAuthHandler};
use maowbot::crypto::Encryptor;
// Removed the unused `Error` import
use maowbot::repositories::sqlite::{
    PlatformIdentityRepository,
    SqliteCredentialsRepository,
    SqliteUserAnalysisRepository,
    UserRepository,
};
use maowbot::cache::{CacheConfig, ChatCache, TrimPolicy};
use maowbot::eventbus::{EventBus, BotEvent};
use maowbot::eventbus::db_logger::spawn_db_logger_task;
use maowbot::platforms::manager::PlatformManager;
use maowbot::repositories::sqlite::analytics::SqliteAnalyticsRepository;
use maowbot::services::message_service::MessageService;
use maowbot::services::user_service::UserService;

/// Command-line arguments
#[derive(Parser, Debug, Clone)]
#[command(name = "maowbot")]
#[command(author, version, about = "MaowBot - multi-platform streaming bot with plugin system")]
struct Args {
    /// Run mode: "server", "client", or "single"
    #[arg(long, default_value = "single")]
    mode: String,

    /// Address of the server (used in server or client mode)
    #[arg(long, default_value = "127.0.0.1:9999")]
    server_addr: String,

    /// Path to the SQLite DB (used only in server or single mode)
    #[arg(long, default_value = "data/bot.db")]
    db_path: String,

    /// Optional passphrase to authenticate plugins
    #[arg(long)]
    plugin_passphrase: Option<String>,

    /// [Optional] If set, load an in-process plugin from the given path
    #[arg(long)]
    in_process_plugin: Option<String>,
}

fn init_tracing() {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(tracing::Level::DEBUG)
        .finish();
    tracing::subscriber::set_global_default(subscriber)
        .expect("Failed to set global default subscriber");
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();

    let args = Args::parse();
    info!("MaowBot starting; mode = {}", args.mode);

    match args.mode.as_str() {
        "server" => {
            // In server mode, we create the EventBus, run the server in a task, and watch Ctrl-C
            let event_bus = Arc::new(EventBus::new());

            // 1) Spawn the server
            let server_handle = {
                let bus_clone = event_bus.clone();
                tokio::spawn(async move {
                    if let Err(e) = run_server(args, bus_clone).await {
                        error!("Server error: {:?}", e);
                    }
                })
            };

            // 2) Watch for Ctrl-C
            let ctrlc_handle = {
                let bus_clone = event_bus.clone();
                tokio::spawn(async move {
                    if let Err(e) = tokio::signal::ctrl_c().await {
                        error!("Failed to listen for Ctrl-C: {:?}", e);
                        return;
                    }
                    info!("Ctrl-C detected, shutting down event_bus...");
                    bus_clone.shutdown();
                })
            };

            // 3) Wait for server or Ctrl-C to finish
            tokio::select! {
                _ = server_handle => { /* server finished or errored */ },
                _ = ctrlc_handle => { /* ctrl-c triggered shutdown */ },
            }
            info!("Main has finished. Goodbye!");
        }

        "client" => {
            // Client mode: just run the client logic, which blocks.
            run_client(args).await?;
            info!("Client mode finished.");
        }

        "single" => {
            // Single-PC mode: old approach that spawns server in background and
            // loads in-process plugin (if provided), then basically never shuts down.
            run_single_pc(args).await?;
            info!("Single-PC mode finished.");
        }

        other => {
            error!("Invalid mode specified: {}", other);
            error!("Valid modes are: server, client, single.");
        }
    }

    Ok(())
}

/// Run only the server: set up DB, plugin manager, background tasks, etc.
async fn run_server(args: Args, event_bus: Arc<EventBus>) -> anyhow::Result<()> {
    // 1) Setup DB
    let db = Database::new(&args.db_path).await?;
    db.migrate().await?;

    // Example usage of monthly maintenance, if you want:
    {
        use maowbot::repositories::sqlite::SqliteUserAnalysisRepository;
        use maowbot::tasks::monthly_maintenance;
        let analysis_repo = SqliteUserAnalysisRepository::new(db.pool().clone());
        if let Err(e) = monthly_maintenance::maybe_run_monthly_maintenance(&db, &analysis_repo).await {
            error!("Monthly maintenance error: {:?}", e);
        }
    }

    let key = [0u8; 32];
    let encryptor = Encryptor::new(&key)?;
    let creds_repo = SqliteCredentialsRepository::new(db.pool().clone(), encryptor);

    // 2) Create (but not currently used) AuthManager
    let _auth_manager = AuthManager::new(
        Box::new(creds_repo.clone()),
        Box::new(StubAuthHandler::default())
    );

    // 3) Create plugin manager
    let mut plugin_manager = PluginManager::new(args.plugin_passphrase.clone());

    // 4) DB Logger task
    let analytics_repo = SqliteAnalyticsRepository::new(db.pool().clone());
    spawn_db_logger_task(&event_bus, analytics_repo, 100, 5);

    // 5) PluginManager subscribes to the bus
    plugin_manager.subscribe_to_event_bus(event_bus.clone());
    plugin_manager.set_event_bus(event_bus.clone());

    // 6) [Optional] If user provided an in-process plugin path, load it:
    if let Some(path) = args.in_process_plugin.as_ref() {
        if let Err(e) = plugin_manager.load_in_process_plugin(path) {
            error!("Failed to load in-process plugin: {:?}", e);
        }
    }

    // 7) Build user manager, message service, platform manager
    let user_repo = UserRepository::new(db.pool().clone());
    let identity_repo = PlatformIdentityRepository::new(db.pool().clone());
    let analysis_repo = SqliteUserAnalysisRepository::new(db.pool().clone());

    let default_user_mgr = DefaultUserManager::new(user_repo, identity_repo, analysis_repo);
    let user_manager = Arc::new(default_user_mgr);
    let user_service = Arc::new(UserService::new(user_manager.clone()));

    let trim_policy = TrimPolicy {
        max_age_seconds: Some(24 * 3600),
        spam_score_cutoff: Some(5.0),
        max_total_messages: Some(10000),
        max_messages_per_user: Some(200),
        min_quality_score: Some(0.2),
    };
    let chat_cache = ChatCache::new(
        SqliteUserAnalysisRepository::new(db.pool().clone()),
        CacheConfig { trim_policy }
    );
    let chat_cache = Arc::new(Mutex::new(chat_cache));
    let message_service = Arc::new(MessageService::new(chat_cache, event_bus.clone()));

    let platform_manager = PlatformManager::new(
        message_service.clone(),
        user_service.clone(),
        event_bus.clone(),
    );
    platform_manager.start_all_platforms().await?;

    // 8) Start listening for external plugin connections in background
    let pm_clone = plugin_manager.clone();
    let server_addr = args.server_addr.clone();
    tokio::spawn(async move {
        if let Err(e) = pm_clone.listen(&server_addr).await {
            error!("PluginManager listen error: {:?}", e);
        }
    });

    // 9) Main “tick” loop until shutdown
    let mut shutdown_rx = event_bus.shutdown_rx.clone();
    loop {
        tokio::select! {
            _ = tokio::time::sleep(Duration::from_secs(10)) => {
                event_bus.publish(BotEvent::Tick).await;
            }
            Ok(_) = shutdown_rx.changed() => {
                if *shutdown_rx.borrow() {
                    info!("run_server sees shutdown => break out of main loop");
                    break;
                }
            }
        }
    }

    info!("run_server is finishing gracefully");
    Ok(())
}

/// Connect to an existing server as a “plugin-like client”.
async fn run_client(args: Args) -> anyhow::Result<()> {
    info!("Running in CLIENT mode...");

    let server_addr: SocketAddr = args.server_addr.parse()?;
    info!("Attempting to connect to MaowBot server at {}", server_addr);

    let stream = TcpStream::connect(server_addr).await?;
    info!("Connected to server at {}", server_addr);

    let (reader, mut writer) = stream.into_split();
    let mut lines = BufReader::new(reader).lines();

    // 1) Send a "Hello" to the bot
    let hello_msg = PluginToBot::Hello {
        plugin_name: "RemoteClient".to_string(),
        passphrase: args.plugin_passphrase.clone(),
    };
    let hello_str = serde_json::to_string(&hello_msg)? + "\n";
    writer.write_all(hello_str.as_bytes()).await?;

    // 2) Launch a task to handle inbound events
    tokio::spawn(async move {
        while let Ok(Some(line)) = lines.next_line().await {
            match serde_json::from_str::<BotToPlugin>(&line) {
                Ok(bot_msg) => match bot_msg {
                    BotToPlugin::Welcome { bot_name } => {
                        info!("Server welcomed us. Bot name: {}", bot_name);
                    }
                    BotToPlugin::ChatMessage { platform, channel, user, text } => {
                        info!("ChatMessage => [{platform}#{channel}] {user}: {text}");
                    }
                    BotToPlugin::Tick => {
                        info!("Received Tick event from the server!");
                    }
                    BotToPlugin::AuthError { reason } => {
                        error!("Received AuthError from server: {}", reason);
                    }
                    BotToPlugin::StatusResponse { connected_plugins, server_uptime } => {
                        info!("StatusResponse => connected_plugins={:?}, server_uptime={}s",
                              connected_plugins, server_uptime);
                    }
                    BotToPlugin::CapabilityResponse(resp) => {
                        info!("Got capability grants: {:?}, denies: {:?}", resp.granted, resp.denied);
                    }
                    BotToPlugin::ForceDisconnect { reason } => {
                        error!("Server forced disconnect: {}", reason);
                        break;
                    }
                },
                Err(e) => {
                    error!("Failed to parse message from server: {} - line was: {}", e, line);
                }
            }
        }
        info!("Client read loop ended.");
    });

    // 3) Meanwhile, in the main client loop, do periodic stuff
    loop {
        tokio::time::sleep(Duration::from_secs(10)).await;
        // Example: send a LogMessage
        let log_msg = PluginToBot::LogMessage {
            text: "RemoteClient is still alive!".to_string(),
        };
        let out = serde_json::to_string(&log_msg)? + "\n";
        writer.write_all(out.as_bytes()).await?;
        writer.flush().await?;
    }
}

/// Run in "single PC" mode: we spin up the server in the background (listening
/// for any remote plugins) and load a plugin *in-process* (instead of connecting
/// to ourselves via TCP). Then we basically never stop.
async fn run_single_pc(args: Args) -> anyhow::Result<()> {
    info!("Running in SINGLE-PC mode...");

    // 1) Spawn the server in a background task
    let server_args = args.clone();
    tokio::spawn(async move {
        let event_bus = Arc::new(EventBus::new());
        if let Err(e) = run_server(server_args, event_bus).await {
            error!("Server error: {:?}", e);
        }
    });

    // 2) Give the server a moment to start listening
    tokio::time::sleep(Duration::from_secs(1)).await;

    info!("Single-PC mode: server is running in background. If --in_process_plugin=PATH was set, it's loaded internally. No local TCP client is used.");

    // 3) Just sleep forever (or do your own logic here)
    loop {
        tokio::time::sleep(Duration::from_secs(3600)).await;
    }
}
