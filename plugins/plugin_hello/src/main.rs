use clap::Parser;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio_stream::StreamExt;
use tokio_stream::wrappers::ReceiverStream; // now available via the tokio-stream dependency
use tonic::transport::{Channel, ClientTlsConfig, Certificate};
use tracing::{error, info};
use tracing_subscriber::FmtSubscriber;

// Import the generated types from the shared maowbot-proto crate.
use maowbot_proto::plugs::{
    plugin_service_client::PluginServiceClient,
    PluginStreamRequest,
    plugin_stream_request::Payload as ReqPayload,
    plugin_stream_response::Payload as RespPayload,
    Hello, LogMessage, RequestCaps, PluginCapability,
};

/// CLI arguments for the plugin.
#[derive(Parser, Debug, Clone)]
struct Args {
    /// The bot server address (e.g., 127.0.0.1:9999)
    #[arg(long, default_value = "127.0.0.1:9999")]
    server_addr: String,

    /// The plugin name as it will identify itself to the bot
    #[arg(long, default_value = "HelloPlugin")]
    plugin_name: String,

    /// Optional passphrase for authentication
    #[arg(long)]
    plugin_passphrase: Option<String>,

    /// Path to the server’s certificate (used for TLS)
    #[arg(long, default_value = "certs/server.crt")]
    ca_cert_path: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing for logging.
    let subscriber = FmtSubscriber::builder()
        .with_max_level(tracing::Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)
        .expect("setting default subscriber failed");

    let args = Args::parse();

    // Build the server URL. We assume TLS so use "https://"
    let server_url = format!("https://{}", args.server_addr);

    // Load the CA certificate to trust the server's self‑signed certificate.
    let ca_cert_pem = std::fs::read(&args.ca_cert_path)?;
    let ca_cert = Certificate::from_pem(ca_cert_pem);
    let tls_config = ClientTlsConfig::new().ca_certificate(ca_cert);

    // Create a Tonic channel configured with TLS.
    let channel = Channel::from_shared(server_url)?
        .tls_config(tls_config)?
        .connect()
        .await?;

    let mut client = PluginServiceClient::new(channel);

    // Create an mpsc channel that will carry our outbound PluginStreamRequest messages.
    let (tx, rx) = mpsc::channel::<PluginStreamRequest>(20);
    let mut response_stream = client
        .start_session(ReceiverStream::new(rx))
        .await?
        .into_inner();

    // Spawn a task to process incoming responses from the bot.
    tokio::spawn(async move {
        while let Some(Ok(resp)) = response_stream.next().await {
            if let Some(payload) = resp.payload {
                match payload {
                    RespPayload::Welcome(w) => {
                        info!("Received Welcome: bot_name = {}", w.bot_name);
                    }
                    RespPayload::AuthError(err) => {
                        error!("Received AuthError: {}", err.reason);
                    }
                    RespPayload::Tick(_) => {
                        info!("Received Tick from server");
                    }
                    RespPayload::ChatMessage(msg) => {
                        info!(
                            "ChatMessage: [{}#{}] {}: {}",
                            msg.platform, msg.channel, msg.user, msg.text
                        );
                    }
                    RespPayload::StatusResponse(s) => {
                        info!(
                            "StatusResponse: uptime={} connected_plugins={:?}",
                            s.server_uptime, s.connected_plugins
                        );
                    }
                    RespPayload::CapabilityResponse(c) => {
                        info!(
                            "CapabilityResponse: granted={:?}, denied={:?}",
                            c.granted, c.denied
                        );
                    }
                    RespPayload::ForceDisconnect(d) => {
                        error!("ForceDisconnect: {}", d.reason);
                        break;
                    }
                }
            }
        }
        info!("Response stream ended.");
    });

    // Send the initial Hello message.
    let hello_msg = PluginStreamRequest {
        payload: Some(ReqPayload::Hello(Hello {
            plugin_name: args.plugin_name.clone(),
            passphrase: args.plugin_passphrase.clone().unwrap_or_default(),
        })),
    };
    tx.send(hello_msg).await?;

    // Send a capabilities request (e.g., asking for SendChat, SceneManagement, and ChatModeration).
    let caps_req = PluginStreamRequest {
        payload: Some(ReqPayload::RequestCaps(RequestCaps {
            requested: vec![
                PluginCapability::SendChat as i32,
                PluginCapability::SceneManagement as i32,
                PluginCapability::ChatModeration as i32,
            ],
        })),
    };
    tx.send(caps_req).await?;

    // Periodically send a log message to indicate the plugin is alive.
    loop {
        tokio::time::sleep(Duration::from_secs(15)).await;
        let log_req = PluginStreamRequest {
            payload: Some(ReqPayload::LogMessage(LogMessage {
                text: format!("Plugin '{}' reporting in.", args.plugin_name),
            })),
        };
        if tx.send(log_req).await.is_err() {
            error!("Failed to send log message; server may be disconnected.");
            break;
        }
    }

    // Even though the loop is expected to run indefinitely,
    // we add an Ok(()) here so that the async main returns a Result.
    Ok(())
}
