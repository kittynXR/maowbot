// src/main.rs

use clap::Parser;
use std::time::Duration;
use std::net::SocketAddr;
use tokio::sync::Mutex;
use tokio::task;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tracing::{error, info};
use tracing_subscriber::FmtSubscriber;
use std::sync::Arc;

//
// Import your crate modules (your code might differ):
//
use maowbot::Database;                 // presumably you have "maowbot" as the crate name
use maowbot::plugins::manager::PluginManager;
use maowbot::plugins::protocol::{BotToPlugin, PluginToBot};
use maowbot::tasks::credential_refresh;
use maowbot::auth::{AuthManager, StubAuthHandler};
use maowbot::crypto::Encryptor;
use maowbot::repositories::sqlite::SqliteCredentialsRepository;
use maowbot::{Error};

use maowbot::eventbus::{EventBus, BotEvent};
use maowbot::eventbus::db_logger::spawn_db_logger_task;
use maowbot::repositories::sqlite::analytics::SqliteAnalyticsRepository;

/// Command-line arguments
#[derive(Parser, Debug, Clone)]
#[command(name = "maowbot")]
#[command(author, version, about = "MaowBot - multi-platform streaming bot with plugin system")]
struct Args {
    /// Run mode: "server", "client", or "single"
    #[arg(long, default_value = "single")]
    mode: String,

    /// Address of the server (used in client mode)
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
        "server" => run_server(args).await?,
        "client" => run_client(args).await?,
        "single" => run_single_pc(args).await?,
        other => {
            error!("Invalid mode specified: {}", other);
            error!("Valid modes are: server, client, single.");
        }
    }

    Ok(())
}

/// Run only the server: set up DB, plugin manager, background tasks, etc.
async fn run_server(args: Args) -> anyhow::Result<()> {
    // 1) Setup DB
    let db = Database::new(&args.db_path).await?;
    db.migrate().await?;
    let key = [0u8; 32];
    let encryptor = Encryptor::new(&key)?;
    let creds_repo = SqliteCredentialsRepository::new(db.pool().clone(), encryptor);

    // Example: create an AuthManager
    let auth_manager = AuthManager::new(
        Box::new(creds_repo.clone()),
        Box::new(StubAuthHandler::default())
    );

    // 2) Create plugin manager
    let plugin_manager = PluginManager::new(args.plugin_passphrase.clone());

    // 3) Create EventBus
    let event_bus = Arc::new(EventBus::new());

    // 4) DB Logger task
    let analytics_repo = SqliteAnalyticsRepository::new(db.pool().clone());
    // Batching with buffer=100, flush_interval=5s
    spawn_db_logger_task(
        &event_bus,
        analytics_repo,
        100,
        5
    );

    // 5) PluginManager subscribes to the bus
    plugin_manager.subscribe_to_event_bus(event_bus.clone());

    // Also set event_bus on plugin manager so it can re-publish plugin messages:
    {
        let mut pm_ref = plugin_manager.clone();
        pm_ref.set_event_bus(event_bus.clone());
    }

    // 6) If user requested an in-process plugin
    if let Some(path) = args.in_process_plugin.as_ref() {
        if let Err(e) = plugin_manager.load_in_process_plugin(path) {
            error!("Failed to load in-process plugin: {:?}", e);
        }
    }

    // 7) Start listening for external plugin connections
    let pm_clone = plugin_manager.clone();
    let server_addr = args.server_addr.clone();
    tokio::spawn(async move {
        if let Err(e) = pm_clone.listen(&server_addr).await {
            error!("PluginManager listen error: {:?}", e);
        }
    });

    // 8) Periodically publish a “Tick” event for demonstration
    loop {
        tokio::time::sleep(std::time::Duration::from_secs(10)).await;
        let evt = BotEvent::Tick;
        event_bus.publish(evt).await;
    }
}

/// Connect to an existing server as a “plugin-like client”
/// (e.g., minimal demonstration of how a plugin might run).
async fn run_client(args: Args) -> anyhow::Result<()> {
    info!("Running in CLIENT mode...");

    let server_addr: SocketAddr = args.server_addr.parse()?;
    info!("Attempting to connect to MaowBot server at {}", server_addr);

    let stream = TcpStream::connect(server_addr).await?;
    info!("Connected to server at {}", server_addr);

    // Split the stream
    let (reader, mut writer) = stream.into_split();
    let mut lines = BufReader::new(reader).lines();

    // 1) Immediately send a "Hello" to the bot
    let hello_msg = PluginToBot::Hello {
        plugin_name: "RemoteClient".to_string(),
        passphrase: args.plugin_passphrase.clone(),
    };
    let hello_str = serde_json::to_string(&hello_msg)? + "\n";
    writer.write_all(hello_str.as_bytes()).await?;

    // 2) Launch a task to handle inbound events from the server
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
                        // Possibly break or handle the error
                    }
                    BotToPlugin::StatusResponse { connected_plugins, server_uptime } => {
                        info!("StatusResponse => connected_plugins={:?}, server_uptime={}s",
                              connected_plugins, server_uptime);
                    }
                    BotToPlugin::CapabilityResponse(resp) => {
                        info!("Got capability grants: {:?}, denies: {:?}",
                              resp.granted, resp.denied);
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

    // 3) Meanwhile, in the main client task, do periodic stuff
    loop {
        tokio::time::sleep(Duration::from_secs(10)).await;
        // Example: send a LogMessage to the server
        let log_msg = PluginToBot::LogMessage {
            text: "RemoteClient is still alive!".to_string(),
        };
        let out = serde_json::to_string(&log_msg)? + "\n";
        writer.write_all(out.as_bytes()).await?;
        writer.flush().await?;
    }
}

/// Run in “single PC” mode, i.e. server + local client in one process.
async fn run_single_pc(args: Args) -> anyhow::Result<()> {
    info!("Running in SINGLE-PC mode...");

    // We'll spawn the server first
    let server_args = args.clone();
    tokio::spawn(async move {
        if let Err(e) = run_server(server_args).await {
            error!("Server task ended with error: {:?}", e);
        }
    });

    // Give the server a moment to start listening
    tokio::time::sleep(Duration::from_secs(1)).await;

    // Then run the local client, connecting to "127.0.0.1:9999" by default
    run_client(args).await
}
