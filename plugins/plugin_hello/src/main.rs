use tokio::signal;
use tonic::transport::{Channel, Certificate, ClientTlsConfig};
use maowbot_proto::plugs::plugin_service_client::PluginServiceClient;
use maowbot_proto::plugs::{PluginStreamRequest, plugin_stream_request::Payload as ReqPayload, plugin_stream_response::Payload as RespPayload, Hello, LogMessage, RequestCaps, PluginCapability};
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use futures_util::StreamExt;
use std::error::Error;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let ca_cert = tokio::fs::read("certs/server.crt").await?;
    let ca_cert = Certificate::from_pem(ca_cert);
    let tls_config = ClientTlsConfig::new()
        .ca_certificate(ca_cert)
        .domain_name("localhost");

    let channel = Channel::from_shared("https://localhost:9999")?
        .tls_config(tls_config)?
        .connect()
        .await?;

    let mut client = PluginServiceClient::new(channel);

    let (tx, rx) = mpsc::channel::<PluginStreamRequest>(10);
    let in_stream = ReceiverStream::new(rx);
    let mut outbound = client.start_session(in_stream).await?.into_inner();

    // Spawn a task to listen for server -> plugin messages.
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
                        println!("Status => connected_plugins={:?}, uptime={}",
                                 status.connected_plugins, status.server_uptime);
                    },
                    RespPayload::CapabilityResponse(caps) => {
                        println!("CapabilityResponse => granted={:?}, denied={:?}",
                                 caps.granted, caps.denied);
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
        println!("Server closed plugin stream.");
    });

    // Send the Hello message.
    let passphrase = ""; // If your server requires one, put it here
    tx.send(PluginStreamRequest {
        payload: Some(ReqPayload::Hello(Hello {
            plugin_name: "HelloGrpc".to_string(),
            passphrase: passphrase.to_string(),
        })),
    }).await?;

    tx.send(PluginStreamRequest {
        payload: Some(ReqPayload::RequestCaps(RequestCaps {
            requested: vec![
                PluginCapability::ReceiveChatEvents as i32,
            ],
        })),
    }).await?;

    // Send a test log message
    tx.send(PluginStreamRequest {
        payload: Some(ReqPayload::LogMessage(LogMessage {
            text: "Hello from plugin_hello, staying alive...".into(),
        })),
    }).await?;

    println!("Connected. Press Ctrl+C to exit.");

    // Wait here until Ctrl+C
    signal::ctrl_c().await?;
    println!("Got Ctrl+C => exiting plugin.");
    Ok(())
}