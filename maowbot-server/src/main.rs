use clap::Parser;
use std::time::Duration;
use std::net::SocketAddr;
use std::sync::Arc;
use std::fs;
use std::path::Path;
use tokio::sync::{Mutex, mpsc};
use tracing::{error, info};
use tracing_subscriber::{fmt, EnvFilter};

use maowbot_core::Database;
use maowbot_core::plugins::manager::{PluginManager, PluginServiceGrpc};
use maowbot_core::eventbus::{EventBus, BotEvent};
use maowbot_core::repositories::postgres::{
    PlatformIdentityRepository,
    PostgresCredentialsRepository,
    PostgresUserAnalysisRepository,
    UserRepository,
};
use maowbot_core::auth::{AuthManager, DefaultUserManager, StubAuthHandler};
use maowbot_core::crypto::Encryptor;
use maowbot_core::cache::{CacheConfig, ChatCache, TrimPolicy};
use maowbot_core::services::message_service::MessageService;
use maowbot_core::services::user_service::UserService;
use maowbot_core::tasks::biweekly_maintenance;
use maowbot_core::tasks::cache_maintenance::spawn_cache_prune_task;
use maowbot_core::tasks::biweekly_maintenance::spawn_biweekly_maintenance_task;

use maowbot_core::plugins::bot_api::BotApi;

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

mod portable_postgres;
use portable_postgres::*;

#[derive(Parser, Debug, Clone)]
#[command(name = "maowbot")]
#[command(author, version, about = "MaowBot - multi‑platform streaming bot with plugin system")]
struct Args {
    /// Mode: "server" or "client"
    #[arg(long, default_value = "server")]
    mode: String,

    /// Address to which the server will bind
    #[arg(long, default_value = "0.0.0.0:9999")]
    server_addr: String,

    /// Postgres connection URL.
    #[arg(long, default_value = "postgres://maow@localhost:5432/maowbot")]
    db_path: String,

    /// Passphrase for plugin connections
    #[arg(long)]
    plugin_passphrase: Option<String>,

    /// Path to an in‑process plugin .so/.dll (optional)
    #[arg(long)]
    in_process_plugin: Option<String>,

    /// If you want to run in headless mode
    #[arg(long, default_value = "false")]
    headless: bool,

    #[arg(long, default_value = "false")]
    auth: bool,
}

fn init_tracing() {
    let filter = EnvFilter::from_default_env()
        .add_directive("maowbot=info".parse().unwrap_or_default());
    let sub = fmt().with_env_filter(filter).finish();
    tracing::subscriber::set_global_default(sub)
        .expect("Failed to set global subscriber");
}

/// The server logic.
async fn run_server(args: Args) -> Result<(), Error> {
    // 1) Start local Postgres if we want to run it ourselves.
    let pg_bin_dir = "./postgres/bin";
    let pg_data_dir = "./postgres/data";
    let port = 5432;

    ensure_db_initialized(pg_bin_dir, pg_data_dir)
        .map_err(|e| Error::Io(e))?;
    start_postgres(pg_bin_dir, pg_data_dir, port)
        .map_err(|e| Error::Io(e))?;

    create_database(pg_bin_dir, port, "maowbot")
        .map_err(|e| Error::Io(e))?;

    // 2) Connect to Postgres
    let db_url = args.db_path.clone();
    info!("Using Postgres DB URL: {}", db_url);
    let db = Database::new(&db_url).await?;
    db.migrate().await?;

    // 3) Initialize event bus & run tasks
    let event_bus = Arc::new(EventBus::new());
    if args.auth {
        info!("`--auth` argument provided; running auth-specific logic as needed.");
    }

    // Spawn the periodic biweekly maintenance background task.
    // (Note: we now pass a proper repository rather than a cutoff number.)
    let _maintenance_handle = spawn_biweekly_maintenance_task(
        db.clone(),
        PostgresUserAnalysisRepository::new(db.pool().clone()),
    );

    // 4) Setup Auth, Repos, PluginManager, etc.
    let key = [0u8; 32];
    let encryptor = Encryptor::new(&key)?;
    let creds_repo = PostgresCredentialsRepository::new(db.pool().clone(), encryptor);
    let _auth_manager = AuthManager::new(
        Box::new(creds_repo.clone()),
        Box::new(StubAuthHandler::default()),
    );

    let mut plugin_manager = PluginManager::new(args.plugin_passphrase.clone());
    plugin_manager.subscribe_to_event_bus(event_bus.clone());
    plugin_manager.set_event_bus(event_bus.clone());

    // Optionally load your in‑process plugin
    if let Some(path) = args.in_process_plugin.as_ref() {
        if let Err(e) = plugin_manager.load_in_process_plugin(path).await {
            error!("Failed to load in‑process plugin from {}: {:?}", path, e);
        }
    }
    // Also load everything in /plugs
    if let Err(e) = plugin_manager.load_plugins_from_folder("plugs").await {
        error!("Failed to load plugins from folder: {:?}", e);
    }

    // Set BotApi for in‑memory plugins
    {
        let api: Arc<dyn BotApi> = Arc::new(plugin_manager.clone());
        let lock = plugin_manager.plugins.lock().await;
        for plugin in lock.iter() {
            plugin.set_bot_api(api.clone());
        }
    }

    let user_repo = UserRepository::new(db.pool().clone());
    let identity_repo = PlatformIdentityRepository::new(db.pool().clone());
    let analysis_repo = PostgresUserAnalysisRepository::new(db.pool().clone());
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
        PostgresUserAnalysisRepository::new(db.pool().clone()),
        CacheConfig { trim_policy },
    );
    let chat_cache = Arc::new(Mutex::new(chat_cache));
    spawn_cache_prune_task(chat_cache.clone(), Duration::from_secs(60));
    let message_service = Arc::new(MessageService::new(chat_cache, event_bus.clone()));

    let platform_manager = maowbot_core::platforms::manager::PlatformManager::new(
        message_service.clone(),
        user_service.clone(),
        event_bus.clone(),
    );
    platform_manager.start_all_platforms().await?;

    // 5) Build & launch the gRPC server
    let identity = load_or_generate_certs()?;
    let tls_config = ServerTlsConfig::new().identity(identity);
    let addr: SocketAddr = args.server_addr.parse()?;
    info!("Starting Tonic gRPC server on {}", addr);
    let service = PluginServiceGrpc { manager: Arc::new(plugin_manager) };
    let server_future = Server::builder()
        .tls_config(tls_config)?
        .add_service(PluginServiceServer::new(service))
        .serve(addr);

    let eb_clone = event_bus.clone();
    let srv_handle = tokio::spawn(async move {
        if let Err(e) = server_future.await {
            error!("gRPC server error: {:?}", e);
        }
    });

    // 6) Handle Ctrl‑C to signal shutdown
    let _ctrlc_handle = tokio::spawn(async move {
        if let Err(e) = tokio::signal::ctrl_c().await {
            error!("Failed to listen for Ctrl‑C: {:?}", e);
        }
        info!("Ctrl‑C detected; shutting down event bus...");
        eb_clone.shutdown();
    });

    // 7) Main event loop
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

    // 8) Stop the gRPC server
    info!("Stopping gRPC server...");
    srv_handle.abort();

    // 9) Stop Postgres
    stop_postgres(pg_bin_dir, pg_data_dir).map_err(|e| Error::Io(e))?;

    Ok(())
}

/// The client logic: ...
async fn run_client(args: Args) -> Result<(), Error> {
    info!("Running in CLIENT mode. Connecting to server...");

    let server_url = format!("https://{}", args.server_addr);
    let ca_cert_pem = fs::read("certs/server.crt")?;
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
                text: "RemoteClient is alive with self‑signed cert!".to_string(),
            })),
        }).await.is_err() {
            error!("Failed to send to server (maybe disconnected).");
            break;
        }
    }

    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    init_tracing();
    let args = Args::parse();
    info!("MaowBot starting. mode={}, headless={}, auth={}", args.mode, args.headless, args.auth);

    let rt = tokio::runtime::Runtime::new()?;
    let result = match args.mode.as_str() {
        "server" => rt.block_on(run_server(args)),
        "client" => rt.block_on(run_client(args)),
        other => {
            error!("Invalid mode '{}'. Use --mode=server or --mode=client", other);
            Ok(())
        }
    };
    info!("Main finished. Goodbye!");
    result.map_err(|e| e.into())
}

/// Utility function to load or generate self‑signed certificates.
fn load_or_generate_certs() -> Result<Identity, Error> {
    use std::io::Write;
    let cert_folder = "certs";
    let cert_path = format!("{}/server.crt", cert_folder);
    let key_path  = format!("{}/server.key", cert_folder);

    if Path::new(&cert_path).exists() && Path::new(&key_path).exists() {
        let cert_pem = fs::read(&cert_path)?;
        let key_pem  = fs::read(&key_path)?;
        return Ok(Identity::from_pem(cert_pem, key_pem));
    }

    let alt_names = vec![
        "localhost".to_string(),
        "127.0.0.1".to_string(),
        "0.0.0.0".to_string(),
    ];

    let CertifiedKey { cert, key_pair } = generate_simple_self_signed(alt_names)?;
    let cert_pem = cert.pem();
    let key_pem  = key_pair.serialize_pem();

    fs::create_dir_all(cert_folder)?;
    fs::File::create(&cert_path)?.write_all(cert_pem.as_bytes())?;
    fs::File::create(&key_path)?.write_all(key_pem.as_bytes())?;

    Ok(Identity::from_pem(cert_pem, key_pem))
}