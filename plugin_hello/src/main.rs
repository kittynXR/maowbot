// plugin_hello/src/main.rs

use clap::Parser;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tokio_rustls::rustls::{ClientConfig, OwnedTrustAnchor, RootCertStore, ServerName};
use tokio_rustls::TlsConnector;
use tracing::{error, info};
use tracing_subscriber::FmtSubscriber;

//
// For demonstration, let's define the same protocol structs/enums
// that the bot uses. In practice, you'd import these from a shared crate.
//
// ------------- Protocol Definitions (Copied Here) -------------
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PluginCapability {
    ReceiveChatEvents,
    SendChat,
    SceneManagement,
    ChatModeration,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestedCapabilities {
    pub requested: Vec<PluginCapability>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrantedCapabilities {
    pub granted: Vec<PluginCapability>,
    pub denied: Vec<PluginCapability>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type", content = "data")]
pub enum BotToPlugin {
    Welcome {
        bot_name: String,
    },
    AuthError {
        reason: String,
    },
    Tick,
    ChatMessage {
        platform: String,
        channel: String,
        user: String,
        text: String,
    },
    StatusResponse {
        connected_plugins: Vec<String>,
        server_uptime: u64,
    },
    CapabilityResponse(GrantedCapabilities),
    ForceDisconnect {
        reason: String,
    },
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum PluginToBot {
    Hello {
        plugin_name: String,
        passphrase: Option<String>,
    },
    LogMessage {
        text: String,
    },
    RequestStatus,
    RequestCapabilities(RequestedCapabilities),
    Shutdown,
    SwitchScene {
        scene_name: String,
    },
    SendChat {
        channel: String,
        text: String,
    },
}
// ------------- End of Copied Protocol Definitions -------------

/// CLI arguments for plugin_hello
#[derive(Parser, Debug, Clone)]
struct Args {
    /// The server address to connect to (plaintext or TLS)
    #[arg(long, default_value = "127.0.0.1:9999")]
    server_addr: String,

    /// If true, connect via TLS
    #[arg(long, default_value = "false")]
    enable_secure_plugins: bool,

    /// Optional passphrase that the server expects
    #[arg(long)]
    plugin_passphrase: Option<String>,

    /// Plugin name as we want to identify ourselves to the server
    #[arg(long, default_value = "HelloPlugin")]
    plugin_name: String,
}

fn init_tracing() {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(tracing::Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)
        .expect("Failed to set tracing subscriber");
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();
    let args = Args::parse();

    info!("Hello Plugin starting. args={:?}", args);

    // Connect (plaintext or TLS)
    if args.enable_secure_plugins {
        connect_via_tls(args).await?;
    } else {
        connect_plaintext(args).await?;
    }
    Ok(())
}

async fn connect_plaintext(args: Args) -> anyhow::Result<()> {
    let addr: SocketAddr = args.server_addr.parse()?;
    info!("(Plaintext) Connecting to {} ...", addr);

    let stream = TcpStream::connect(addr).await?;
    info!("Connected (plaintext) to {}", addr);

    plugin_main_loop(stream, args).await
}

async fn connect_via_tls(args: Args) -> anyhow::Result<()> {
    let addr: SocketAddr = args.server_addr.parse()?;
    info!("(TLS) Connecting to {} ...", addr);

    // Minimal root store
    let mut root_store = RootCertStore::empty();
    // For real usage, load a CA or trust anchors
    // But here we just trust none => not secure in production

    let config = ClientConfig::builder()
        .with_safe_defaults()
        .with_root_certificates(root_store)
        .with_no_client_auth();

    let connector = TlsConnector::from(std::sync::Arc::new(config));

    let tcp = TcpStream::connect(addr).await?;
    info!("TCP connected, starting TLS handshake...");

    let domain = ServerName::try_from("localhost").map_err(|_| anyhow::anyhow!("Invalid domain"))?;
    let tls_stream = connector.connect(domain, tcp).await?;
    info!("TLS handshake successful!");

    plugin_main_loop(tls_stream, args).await
}

async fn plugin_main_loop<S>(stream: S, args: Args) -> anyhow::Result<()>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static,
{
    let (reader, writer) = tokio::io::split(stream);
    let mut writer = tokio::io::BufWriter::new(writer);
    let mut lines = BufReader::new(reader).lines();

    // 1) Send "Hello" with optional passphrase
    let hello_msg = PluginToBot::Hello {
        plugin_name: args.plugin_name.clone(),
        passphrase: args.plugin_passphrase.clone(),
    };
    let hello_str = serde_json::to_string(&hello_msg)? + "\n";
    writer.write_all(hello_str.as_bytes()).await?;

    // 2) Immediately request some capabilities
    let capabilities_req = PluginToBot::RequestCapabilities(RequestedCapabilities {
        requested: vec![
            PluginCapability::SendChat,
            PluginCapability::SceneManagement,
            PluginCapability::ChatModeration, // probably will be denied
        ],
    });
    let cap_req_str = serde_json::to_string(&capabilities_req)? + "\n";
    writer.write_all(cap_req_str.as_bytes()).await?;

    writer.flush().await?;

    // Spawn task to read incoming lines from server
    tokio::spawn({
        let plugin_name = args.plugin_name.clone();
        async move {
            while let Ok(Some(line)) = lines.next_line().await {
                match serde_json::from_str::<BotToPlugin>(&line) {
                    Ok(msg) => {
                        match msg {
                            BotToPlugin::Welcome { bot_name } => {
                                info!("Server welcomed plugin '{}': bot_name={}", plugin_name, bot_name);
                            }
                            BotToPlugin::AuthError { reason } => {
                                error!("AuthError from server: {}. Closing.", reason);
                                break;
                            }
                            BotToPlugin::Tick => {
                                info!("[{}] Received Tick event!", plugin_name);
                            }
                            BotToPlugin::ChatMessage { platform, channel, user, text } => {
                                info!("[{}] ChatMessage => [{platform}#{channel}] {user}: {text}",
                                      plugin_name);
                            }
                            BotToPlugin::StatusResponse { connected_plugins, server_uptime } => {
                                info!("[{}] Status => connected_plugins={:?}, server_uptime={}s",
                                      plugin_name, connected_plugins, server_uptime);
                            }
                            BotToPlugin::CapabilityResponse(grants) => {
                                info!("[{}] Capabilities granted: {:?}, denied: {:?}",
                                      plugin_name, grants.granted, grants.denied);
                            }
                            BotToPlugin::ForceDisconnect { reason } => {
                                error!("[{}] ForceDisconnect from server: {}", plugin_name, reason);
                                break;
                            }
                        }
                    }
                    Err(e) => {
                        error!("[{}] Failed to parse line: {} -- {}", plugin_name, e, line);
                    }
                }
            }
            info!("[{}] Read loop ended (disconnected).", plugin_name);
        }
    });

    // Main plugin loop: periodically send a LogMessage, plus request status
    loop {
        tokio::time::sleep(std::time::Duration::from_secs(15)).await;

        // 1) LogMessage
        let log = PluginToBot::LogMessage {
            text: format!("Hello from plugin '{}' - I'm alive!", args.plugin_name),
        };
        let log_str = serde_json::to_string(&log)? + "\n";
        writer.write_all(log_str.as_bytes()).await?;

        // 2) RequestStatus
        let req_status = PluginToBot::RequestStatus;
        let req_str = serde_json::to_string(&req_status)? + "\n";
        writer.write_all(req_str.as_bytes()).await?;

        writer.flush().await?;
    }
}
