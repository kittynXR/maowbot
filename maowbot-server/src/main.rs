// src/main.rs

use clap::Parser;
use std::time::Duration;
use std::net::SocketAddr;
use std::sync::Arc;
use std::fs;
use std::path::Path;
use tokio::sync::{Mutex, mpsc};
use tracing::{error, info};
use tracing_subscriber::FmtSubscriber;

use maowbot_core::Database;
use maowbot_core::plugins::manager::{PluginManager, PluginServiceGrpc};
use maowbot_core::eventbus::{EventBus, BotEvent};
use maowbot_core::repositories::sqlite::{
    PlatformIdentityRepository,
    SqliteCredentialsRepository,
    SqliteUserAnalysisRepository,
    UserRepository,
};
use maowbot_core::auth::{AuthManager, DefaultUserManager, StubAuthHandler};
use maowbot_core::crypto::Encryptor;
use maowbot_core::cache::{CacheConfig, ChatCache, TrimPolicy};
use maowbot_core::services::message_service::MessageService;
use maowbot_core::services::user_service::UserService;
use maowbot_core::tasks::monthly_maintenance;
use maowbot_core::tasks::cache_maintenance::spawn_cache_prune_task;

use tonic::transport::{Server, Identity, Certificate, ServerTlsConfig, Channel, ClientTlsConfig};
use maowbot_proto::plugs::plugin_service_server::PluginServiceServer;
use maowbot_proto::plugs::{
    plugin_service_client::PluginServiceClient,
    PluginStreamRequest,
    plugin_stream_request::Payload as ReqPayload,
    plugin_stream_response::Payload as RespPayload,
    LogMessage, Hello,
};
use tokio_stream::wrappers::ReceiverStream;
use futures_util::StreamExt;

use rcgen::{generate_simple_self_signed, CertifiedKey};
use tokio::time;
use maowbot_core::Error;
use maowbot_core::plugins::bot_api::BotApi;

/// Command-line arguments
#[derive(Parser, Debug, Clone)]
#[command(name = "maowbot")]
#[command(author, version, about = "MaowBot - multi-platform streaming bot with plugin system")]
struct Args {
    #[arg(long, default_value = "server")]
    mode: String,

    #[arg(long, default_value = "127.0.0.1:9999")]
    server_addr: String,

    #[arg(long, default_value = "data/bot.db")]
    db_path: String,

    #[arg(long)]
    plugin_passphrase: Option<String>,

    // We interpret this path as a path to a dynamic library .so/.dll
    #[arg(long)]
    in_process_plugin: Option<String>,

    #[arg(long, default_value = "false")]
    headless: bool,
}

fn init_tracing() {
    let sub = FmtSubscriber::builder()
        .with_max_level(tracing::Level::DEBUG)
        .finish();
    tracing::subscriber::set_global_default(sub)
        .expect("Failed to set global subscriber");
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    init_tracing();

    let args = Args::parse();
    info!("MaowBot starting. mode={}, headless={}", args.mode, args.headless);

    match args.mode.as_str() {
        "server" => run_server(args).await?,
        "client" => run_client(args).await?,
        other => {
            error!("Invalid mode '{}'. Use --mode=server or --mode=client", other);
        }
    }

    info!("Main finished. Goodbye!");
    Ok(())
}

/// Utility function to load or generate self-signed certificates
fn load_or_generate_certs() -> Result<Identity, Error> {
    let cert_folder = "certs";
    let cert_path = format!("{}/server.crt", cert_folder);
    let key_path  = format!("{}/server.key", cert_folder);

    if Path::new(&cert_path).exists() && Path::new(&key_path).exists() {
        let cert_pem = fs::read(&cert_path)?;
        let key_pem  = fs::read(&key_path)?;
        return Ok(Identity::from_pem(cert_pem, key_pem));
    }

    let CertifiedKey { cert, key_pair } = generate_simple_self_signed(vec!["localhost".to_string()])?;
    let cert_pem = cert.pem();
    let key_pem  = key_pair.serialize_pem();

    fs::create_dir_all(cert_folder)?;
    fs::write(&cert_path, cert_pem.as_bytes())?;
    fs::write(&key_path, key_pem.as_bytes())?;

    Ok(Identity::from_pem(cert_pem, key_pem))
}

/// The server logic.
///
/// This function performs the following:
/// - Initializes the event bus and database.
/// - Creates the PluginManager, subscribes it to the event bus, and loads plugins:
///   - Loads a plugin specified by a command-line flag (if provided).
///   - Scans a designated folder (here `"plugs"`) and loads all dynamic libraries found.
/// - Initializes other services (user management, message service, platform manager).
/// - Starts the gRPC server with TLS and a Ctrl‑C shutdown handler.
/// - Enters a loop publishing Tick events until shutdown.
pub async fn run_server(args: Args) -> Result<(), Error> {
    let event_bus = Arc::new(EventBus::new());
    let db = Database::new(&args.db_path).await?;
    db.migrate().await?;

    // Run monthly maintenance as an example.
    {
        let repo = SqliteUserAnalysisRepository::new(db.pool().clone());
        if let Err(e) = monthly_maintenance::maybe_run_monthly_maintenance(&db, &repo).await {
            error!("Monthly maintenance error: {:?}", e);
        }
    }

    // Set up encryption and authentication.
    let key = [0u8; 32];
    let encryptor = Encryptor::new(&key)?;
    let creds_repo = SqliteCredentialsRepository::new(db.pool().clone(), encryptor);
    let _auth_manager = AuthManager::new(
        Box::new(creds_repo.clone()),
        Box::new(StubAuthHandler::default()),
    );

    // Create the PluginManager.
    let mut plugin_manager = PluginManager::new(args.plugin_passphrase.clone());
    plugin_manager.subscribe_to_event_bus(event_bus.clone());
    plugin_manager.set_event_bus(event_bus.clone());

    // Load plugins:
    if let Some(path) = args.in_process_plugin.as_ref() {
        if let Err(e) = plugin_manager.load_in_process_plugin(path).await {
            error!("Failed to load in-process plugin from {}: {:?}", path, e);
        }
    }
    if let Err(e) = plugin_manager.load_plugins_from_folder("plugs").await {
        error!("Failed to load plugins from folder: {:?}", e);
    }

    // Set the BotApi for plugins.
    {
        // Clone plugin_manager so that we can pass it as a BotApi.
        let api: Arc<dyn BotApi> = Arc::new(plugin_manager.clone());
        {
            // Create a separate scope for the lock so it is dropped before moving plugin_manager.
            let lock = plugin_manager.plugins.lock().await;
            for plugin in lock.iter() {
                plugin.set_bot_api(api.clone());
            }
        } // Lock guard dropped here.
    }

    // Build additional services.
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
        CacheConfig { trim_policy },
    );
    let chat_cache = Arc::new(tokio::sync::Mutex::new(chat_cache));
    // e.g. run pruning every 60 seconds
    spawn_cache_prune_task(chat_cache.clone(), std::time::Duration::from_secs(60));
    let message_service = Arc::new(MessageService::new(chat_cache, event_bus.clone()));

    let platform_manager = maowbot_core::platforms::manager::PlatformManager::new(
        message_service.clone(),
        user_service.clone(),
        event_bus.clone(),
    );
    platform_manager.start_all_platforms().await?;

    // Load or generate TLS certificates.
    let identity = load_or_generate_certs()?;
    let tls_config = ServerTlsConfig::new().identity(identity);

    // Parse the server address.
    let addr: SocketAddr = args.server_addr.parse()?;
    info!("Starting Tonic gRPC server on {}", addr);

    // Now that no lock is active, we can move plugin_manager into an Arc.
    let service = PluginServiceGrpc { manager: Arc::new(plugin_manager) };

    let server_future = Server::builder()
        .tls_config(tls_config)?
        .add_service(maowbot_proto::plugs::plugin_service_server::PluginServiceServer::new(service))
        .serve(addr);

    let eb_clone = event_bus.clone();
    let srv_handle = tokio::spawn(async move {
        if let Err(e) = server_future.await {
            error!("gRPC server error: {:?}", e);
        }
    });

    // Spawn a Ctrl-C listener that shuts down the event bus.
    let _ctrlc_handle = tokio::spawn(async move {
        if let Err(e) = tokio::signal::ctrl_c().await {
            error!("Failed to listen for Ctrl-C: {:?}", e);
        }
        info!("Ctrl-C detected; shutting down event bus...");
        eb_clone.shutdown();
    });

    // Main loop: publish Tick events every 10 seconds until shutdown.
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

    info!("Stopping gRPC server...");
    srv_handle.abort();
    Ok(())
}

/// The client logic: it must trust the `server.crt` that the bot generates
async fn run_client(args: Args) -> Result<(), Error> {
    info!("Running in CLIENT mode. Connecting to server...");

    let server_url = format!("https://{}", args.server_addr);
    let ca_cert_pem = std::fs::read("certs/server.crt")?;
    let ca_cert = Certificate::from_pem(ca_cert_pem);

    let tls_config = ClientTlsConfig::new().ca_certificate(ca_cert);

    let channel = Channel::from_shared(server_url)?
        .tls_config(tls_config)?
        .connect()
        .await?;

    let mut client = PluginServiceClient::new(channel);
    let (tx, rx) = mpsc::channel::<PluginStreamRequest>(20);
    let in_stream = ReceiverStream::new(rx);
    let mut outbound = client.start_session(in_stream).await?.into_inner();

    tokio::spawn(async move {
        while let Some(Ok(resp)) = outbound.next().await {
            if let Some(payload) = resp.payload {
                match payload {
                    RespPayload::Welcome(w) => {
                        info!("Server welcomed us => Bot name: {}", w.bot_name);
                    }
                    RespPayload::AuthError(err) => {
                        error!("AuthError => {}", err.reason);
                    }
                    RespPayload::Tick(_) => {
                        info!("Received Tick from server");
                    }
                    RespPayload::ChatMessage(msg) => {
                        info!("(Chat) platform={} channel={} user={} => '{}'",
                              msg.platform, msg.channel, msg.user, msg.text);
                    }
                    RespPayload::StatusResponse(s) => {
                        info!("Status => connected={:?}, uptime={}", s.connected_plugins, s.server_uptime);
                    }
                    RespPayload::CapabilityResponse(c) => {
                        info!("Capabilities => granted={:?}, denied={:?}", c.granted, c.denied);
                    }
                    RespPayload::ForceDisconnect(d) => {
                        error!("Server forced disconnect => {}", d.reason);
                        break;
                    }
                }
            }
        }
        info!("Server->client stream ended.");
    });

    let plugin_pass = args.plugin_passphrase.clone().unwrap_or_default();
    tx.send(PluginStreamRequest {
        payload: Some(ReqPayload::Hello(Hello {
            plugin_name: "RemoteClient".into(),
            passphrase: plugin_pass,
        })),
    }).await?;

    loop {
        tokio::time::sleep(Duration::from_secs(10)).await;
        if tx.send(PluginStreamRequest {
            payload: Some(ReqPayload::LogMessage(LogMessage {
                text: "RemoteClient is alive with self-signed cert!".to_string(),
            })),
        }).await.is_err() {
            error!("Failed to send => server (maybe disconnected).");
            break;
        }
    }

    Ok(())
}