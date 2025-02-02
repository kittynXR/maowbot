use clap::Parser;
use tokio::signal;
use tonic::transport::{Channel, Certificate, ClientTlsConfig};
use maowbot_proto::plugs::plugin_service_client::PluginServiceClient;
use maowbot_proto::plugs::{
    PluginStreamRequest,
    plugin_stream_request::Payload as ReqPayload,
    plugin_stream_response::Payload as RespPayload,
    Hello, LogMessage, RequestCaps, PluginCapability
};
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use futures_util::StreamExt;
use std::error::Error;

/// Command-line arguments for plugin_hello
#[derive(Parser, Debug)]
#[command(author, version, about="plugin_hello gRPC plugin example")]
struct HelloArgs {
    /// The server IP:port to connect to over TLS
    #[arg(long, default_value = "localhost:9999")]
    server_ip: String,

    /// If your bot server requires a plugin passphrase, supply it here
    #[arg(long, default_value = "")]
    passphrase: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let args = HelloArgs::parse();

    // Load the same self-signed cert that the server wrote to certs/server.crt
    let ca_cert = tokio::fs::read("certs/server.crt").await?;
    let ca_cert = Certificate::from_pem(ca_cert);
    let tls_config = ClientTlsConfig::new()
        .ca_certificate(ca_cert)
        // We can leave domain_name as "localhost" or omit it; gRPC uses SNI.
        .domain_name("localhost");

    // Build a channel to e.g. "https://192.168.1.25:9999" or "https://localhost:9999"
    let server_url = format!("https://{}", args.server_ip);
    println!("plugin_hello connecting to {}", server_url);

    let channel = Channel::from_shared(server_url)?
        .tls_config(tls_config)?
        .connect()
        .await?;

    let mut client = PluginServiceClient::new(channel);

    // We set up a channel for sending requests to the server stream:
    let (tx, rx) = mpsc::channel::<PluginStreamRequest>(10);
    let in_stream = ReceiverStream::new(rx);
    let mut outbound = client.start_session(in_stream).await?.into_inner();

    // Spawn a task to listen for server -> plugin messages
    tokio::spawn(async move {
        while let Some(Ok(response)) = outbound.next().await {
            if let Some(payload) = response.payload {
                match payload {
                    RespPayload::Welcome(welcome) => {
                        println!("Server welcomed plugin: bot_name={}", welcome.bot_name);
                    },
                    RespPayload::Tick(_) => {
                        println!("Received Tick from server");
                    },
                    RespPayload::ChatMessage(msg) => {
                        println!(
                            "ChatMessage => [{} #{}] {}: {}",
                            msg.platform, msg.channel, msg.user, msg.text
                        );
                    },
                    RespPayload::StatusResponse(status) => {
                        println!(
                            "Status => connected_plugins={:?}, uptime={}",
                            status.connected_plugins, status.server_uptime
                        );
                    },
                    RespPayload::CapabilityResponse(caps) => {
                        println!(
                            "CapabilityResponse => granted={:?}, denied={:?}",
                            caps.granted, caps.denied
                        );
                    },
                    RespPayload::AuthError(err) => {
                        eprintln!("AuthError => {}", err.reason);
                    },
                    RespPayload::ForceDisconnect(fd) => {
                        eprintln!("ForceDisconnect => {}", fd.reason);
                        break;
                    },
                }
            }
        }
        println!("Server closed plugin stream or disconnected.");
    });

    // Send Hello with optional passphrase
    tx.send(PluginStreamRequest {
        payload: Some(ReqPayload::Hello(Hello {
            plugin_name: "HelloGrpc".to_string(),
            passphrase: args.passphrase.clone(),
        })),
    }).await?;

    // Request minimal capability (just to see how the server responds):
    tx.send(PluginStreamRequest {
        payload: Some(ReqPayload::RequestCaps(RequestCaps {
            requested: vec![PluginCapability::ReceiveChatEvents as i32],
        })),
    }).await?;

    // Send a test log message
    tx.send(PluginStreamRequest {
        payload: Some(ReqPayload::LogMessage(LogMessage {
            text: "Hello from plugin_hello, staying alive...".into(),
        })),
    }).await?;

    println!("Plugin connected. Press Ctrl+C to exit or kill the process...");

    // Wait here until Ctrl+C
    signal::ctrl_c().await?;
    println!("Got Ctrl+C => exiting plugin.");
    Ok(())
}