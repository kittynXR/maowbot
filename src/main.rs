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
        .expect("Failed to set global default subscriber for tracing");
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
    info!("Running in SERVER mode...");

    // 1) Initialize the database
    let db = Database::new(&args.db_path).await?;
    db.migrate().await?;
    info!("Database initialized and migrated successfully!");

    // 2) Create the encryption key (placeholder: all zeros)
    let key = [0u8; 32];
    let encryptor = Encryptor::new(&key)?;

    // 3) Create the credentials repository
    let creds_repo = SqliteCredentialsRepository::new(db.pool().clone(), encryptor);

    // 4) Create AuthManager
    let auth_manager = AuthManager::new(
        Box::new(creds_repo.clone()), // pass a clone if needed
        Box::new(StubAuthHandler::default())
    );
    let auth_manager = Arc::new(Mutex::new(auth_manager));

    // 5) Spawn a background task to refresh credentials
    {
        let creds_repo_clone = creds_repo.clone();
        let auth_manager_clone = auth_manager.clone();

        task::spawn(async move {
            let check_interval = Duration::from_secs(300); // 5 minutes
            loop {
                let within_minutes = 10;
                let mut am = auth_manager_clone.lock().await;
                match credential_refresh::refresh_expiring_tokens(
                    &creds_repo_clone,
                    &mut am,
                    within_minutes
                ).await {
                    Ok(_) => info!("Finished refresh_expiring_tokens cycle."),
                    Err(e) => error!("Error refreshing tokens: {:?}", e),
                }
                tokio::time::sleep(check_interval).await;
            }
        });
    }

    // 6) Create plugin manager
    let plugin_manager = PluginManager::new(args.plugin_passphrase.clone());

    // 6b) If user wants to load an in-process plugin, do it here
    if let Some(path) = args.in_process_plugin.as_ref() {
        match plugin_manager.load_in_process_plugin(path) {
            Ok(_) => info!("Successfully loaded in-process plugin from '{}'", path),
            Err(e) => error!("Failed to load in-process plugin: {:?}", e),
        }
    }

    // 7) Start listening for plugin connections (plaintext).
    //    You could also use plugin_manager.listen_secure(...) if you want TLS.
    tokio::spawn({
        let pm_clone = plugin_manager.clone();
        async move {
            let listen_addr = "0.0.0.0:9999";
            info!("Server listening for plugins on {}", listen_addr);
            if let Err(e) = pm_clone.listen(listen_addr).await {
                error!("PluginManager error: {:?}", e);
            }
        }
    });

    // 8) Main loop: periodically broadcast Tick events
    info!("Server setup complete. Entering main loop...");
    loop {
        tokio::time::sleep(Duration::from_secs(10)).await;
        plugin_manager.broadcast(BotToPlugin::Tick);
        // Could also do other server logic here
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
