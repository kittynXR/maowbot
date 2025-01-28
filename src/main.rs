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

use maowbot::Database;
use maowbot::plugins::manager::PluginManager;
use maowbot::plugins::protocol::{BotToPlugin, PluginToBot};
use maowbot::auth::{AuthManager, DefaultUserManager, StubAuthHandler};
use maowbot::crypto::Encryptor;
use maowbot::repositories::sqlite::{
    PlatformIdentityRepository,
    SqliteCredentialsRepository,
    SqliteUserAnalysisRepository,
    UserRepository,
    analytics::SqliteAnalyticsRepository,
};
use maowbot::cache::{CacheConfig, ChatCache, TrimPolicy};
use maowbot::eventbus::{EventBus, BotEvent};
use maowbot::eventbus::db_logger::spawn_db_logger_task;
use maowbot::platforms::manager::PlatformManager;
use maowbot::plugins::tui_plugin::TuiPlugin;
use maowbot::services::message_service::MessageService;
use maowbot::services::user_service::UserService;
use maowbot::tasks::monthly_maintenance;

/// Command-line arguments
#[derive(Parser, Debug, Clone)]
#[command(name = "maowbot")]
#[command(author, version, about = "MaowBot - multi-platform streaming bot with plugin system")]
struct Args {
    /// Run mode: "server" or "client"
    #[arg(long, default_value = "server")]
    mode: String,

    /// Address of the server (used in server or client mode)
    #[arg(long, default_value = "127.0.0.1:9999")]
    server_addr: String,

    /// Path to the SQLite DB (used only in server mode)
    #[arg(long, default_value = "data/bot.db")]
    db_path: String,

    /// Optional passphrase to authenticate plugins
    #[arg(long)]
    plugin_passphrase: Option<String>,

    /// If set, load an in-process plugin from the given path (server mode only)
    #[arg(long)]
    in_process_plugin: Option<String>,

    /// If set, do not spawn the local TUI. Suitable for Windows/Unix services.
    #[arg(long, default_value = "false")]
    headless: bool,
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

    // "args" that you want to keep using later
    let args = Args::parse();


    info!("MaowBot starting; mode = {}, headless={}", args.mode, args.headless);

    match args.mode.as_str() {
        "server" => {
            let event_bus = Arc::new(EventBus::new());
            // spawn server logic:
            let server_handle = {
                let bus_clone = event_bus.clone();
                tokio::spawn(async move {
                    if let Err(e) = run_server(args.clone(), bus_clone).await {
                        error!("Server error: {:?}", e);
                    }
                })
            };

            // If not headless, create a local TUI plugin:
            // We create the plugin manager FIRST in run_server, so we must wait a bit or pass it out.
            // Simpler approach: we pass plugin info into the server.
            // In practice we’ll do it inside run_server.
            // But to keep it consistent, we do it here after a short delay or by a channel.
            // For simplicity, let’s do the TUI inside run_server right after plugin manager is ready.
            // (See the code below in run_server for actually registering TuiPlugin if !headless.)

            // watch for Ctrl-C
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

            tokio::select! {
                _ = server_handle => { /* server finished or errored */ },
                _ = ctrlc_handle => { /* ctrl-c triggered shutdown */ },
            }
            info!("Main has finished (server). Goodbye!");
        }
        "client" => {
            // run client logic
            let event_bus = Arc::new(EventBus::new());
            // If not headless, we also load a TuiPlugin that can show remote status
            // But we need a plugin_manager to do that. So let’s create a small manager
            // or store a passphrase. We can also skip manager if we just want TUI local logs.
            // For demonstration, we will do a minimal approach.
            // We'll run the client function, then keep TUI if not headless in parallel.

            // Make a separate clone to move into the async task
            let client_args = args.clone();
            let client_handle = tokio::spawn(async move {
                // Now we only move "client_args" into this closure
                if let Err(e) = run_client(client_args).await {
                    error!("Client error: {:?}", e);
                }
            });

            // Also watch Ctrl-C
            let ctrlc_handle = {
                let bus_clone = event_bus.clone();
                tokio::spawn(async move {
                    if let Err(e) = tokio::signal::ctrl_c().await {
                        error!("Failed to listen for Ctrl-C: {:?}", e);
                        return;
                    }
                    info!("Ctrl-C in client mode => shutting down bus...");
                    bus_clone.shutdown();
                })
            };

            if !args.headless {

                // Here, "args" is still in scope and not moved
                let mut local_pm = PluginManager::new(args.plugin_passphrase.clone());

                local_pm.set_event_bus(event_bus.clone());

                // We add the TuiPlugin
                // (We must define the TuiPlugin in a separate module, see tui_plugin.rs)

                let tui_plugin = TuiPlugin::new(Arc::new(local_pm), event_bus.clone());
                // The user’s TUI is now running. We do nothing else here; it’s in the background.
            }

            tokio::select! {
                _ = client_handle => {},
                _ = ctrlc_handle => {},
            }
            info!("Main has finished (client). Goodbye!");
        }
        other => {
            error!("Invalid mode specified: {}", other);
            error!("Valid modes are: server, client.");
        }
    }

    Ok(())
}

/// The core server logic
async fn run_server(args: Args, event_bus: Arc<EventBus>) -> anyhow::Result<()> {
    let db = Database::new(&args.db_path).await?;
    db.migrate().await?;

    // Example monthly maintenance
    {
        let analysis_repo = SqliteUserAnalysisRepository::new(db.pool().clone());
        if let Err(e) = monthly_maintenance::maybe_run_monthly_maintenance(&db, &analysis_repo).await {
            error!("Monthly maintenance error: {:?}", e);
        }
    }

    let key = [0u8; 32];
    let encryptor = Encryptor::new(&key)?;
    let creds_repo = SqliteCredentialsRepository::new(db.pool().clone(), encryptor);

    let _auth_manager = AuthManager::new(
        Box::new(creds_repo.clone()),
        Box::new(StubAuthHandler::default())
    );

    // plugin manager
    let mut plugin_manager = PluginManager::new(args.plugin_passphrase.clone());
    plugin_manager.subscribe_to_event_bus(event_bus.clone());
    plugin_manager.set_event_bus(event_bus.clone());

    // DB logger
    let analytics_repo = SqliteAnalyticsRepository::new(db.pool().clone());
    spawn_db_logger_task(&event_bus, analytics_repo, 100, 5);

    // Optionally load an in-process plugin
    if let Some(path) = args.in_process_plugin.as_ref() {
        if let Err(e) = plugin_manager.load_in_process_plugin(path) {
            error!("Failed to load in-process plugin: {:?}", e);
        }
    }

    // Build user manager, etc.
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

    // Start listening for external plugins automatically (the requirement
    // is that server mode always listens)
    // But if we want to let TUI start/stop it, then skip here.
    // By default we’ll start now:
    plugin_manager.start_listening().await?;

    // If not headless => register TuiPlugin for local REPL:
    if !args.headless {
        let tui_plugin = TuiPlugin::new(Arc::new(plugin_manager.clone()), event_bus.clone());
        // This plugin runs in background. No further calls needed.
        // It’s included in manager’s list once constructed (we do that next):
        {
            let mut lock = plugin_manager.plugins.lock().await;
            lock.push(Arc::new(tui_plugin));
        }
    }

    // main server loop until shutdown
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

    info!("run_server finishing gracefully");
    Ok(())
}

/// The client logic: connect to remote server as a plugin.
async fn run_client(args: Args) -> anyhow::Result<()> {
    info!("Running in CLIENT mode...");

    let server_addr: SocketAddr = args.server_addr.parse()?;
    info!("Attempting to connect to MaowBot server at {}", server_addr);

    let stream = TcpStream::connect(server_addr).await?;
    info!("Connected to server at {}", server_addr);

    let (reader, mut writer) = stream.into_split();
    let mut lines = BufReader::new(reader).lines();

    // 1) Send "Hello"
    let hello_msg = PluginToBot::Hello {
        plugin_name: "RemoteClient".to_string(),
        passphrase: args.plugin_passphrase.clone(),
    };
    let hello_str = serde_json::to_string(&hello_msg)? + "\n";
    writer.write_all(hello_str.as_bytes()).await?;

    // 2) Read inbound events
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
                        info!("Received Tick from server!");
                    }
                    BotToPlugin::AuthError { reason } => {
                        error!("Server AuthError: {}", reason);
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
                    error!("Invalid message from server: {} - line={}", e, line);
                }
            }
        }
        info!("Client read loop ended.");
    });

    // 3) Do periodic keep-alive or logs
    loop {
        tokio::time::sleep(Duration::from_secs(10)).await;
        let log_msg = PluginToBot::LogMessage {
            text: "RemoteClient is still alive!".to_string(),
        };
        let out = serde_json::to_string(&log_msg)? + "\n";
        writer.write_all(out.as_bytes()).await?;
        writer.flush().await?;
    }
}
