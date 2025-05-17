//! maowbot-server/src/client.rs
//!
//! The logic for connecting as a "client" plugin to a remote MaowBot server.

use std::time::Duration;
use tokio::{sync::mpsc, time};
use tonic::transport::{Channel, ClientTlsConfig, Certificate};
use futures_util::StreamExt;
use tokio_stream::wrappers::ReceiverStream;
use tracing::{info, error};
use std::fs;

use maowbot_core::Error;
use maowbot_proto::plugs::{
    plugin_service_client::PluginServiceClient,
    PluginStreamRequest,
    plugin_stream_request::Payload as ReqPayload,
    plugin_stream_response::Payload as RespPayload,
    Hello, LogMessage,
};

use crate::Args;

/// Connects to a remote MaowBot server and acts like a plugin, streaming messages.
pub async fn run_client(args: Args) -> Result<(), Error> {
    info!("Running in CLIENT mode. Connecting to server...");

    let server_url = format!("https://{}", args.server_addr);
    // Load CA from local "certs/server.crt"
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

    // Spawn a task that processes inbound messages from server
    tokio::spawn(async move {
        while let Some(Ok(resp)) = outbound.next().await {
            if let Some(payload) = resp.payload {
                match payload {
                    RespPayload::Welcome(w) => {
                        info!("Server welcomed us => Bot name: {}", w.bot_name);
                    }
                    RespPayload::AuthError(a) => {
                        error!("AuthError => {}", a.reason);
                    }
                    RespPayload::Tick(_) => {
                        info!("Received Tick from server");
                    }
                    RespPayload::ChatMessage(msg) => {
                        info!(
                            "(Chat) platform={} channel={} user={} => '{}'",
                            msg.platform, msg.channel, msg.user, msg.text
                        );
                    }
                    RespPayload::StatusResponse(st) => {
                        info!("Status => connected_plugins={:?}, uptime={}", st.connected_plugins, st.server_uptime);
                    }
                    RespPayload::CapabilityResponse(cr) => {
                        info!("Capabilities => granted={:?}, denied={:?}", cr.granted, cr.denied);
                    }
                    RespPayload::ForceDisconnect(fd) => {
                        error!("Server forced disconnect => {}", fd.reason);
                        break;
                    }
                    RespPayload::GameEvent(ge) => {
                        println!("GameEvent => {}: {}", ge.name, ge.json);
                    },
                    // Fallback if prost adds more variants in future:
                    _ => {
                        println!("Received unknown plugin response variant.");
                    }
                }
            }
        }
        info!("Server->client stream ended.");
    });

    // Send Hello
    let plugin_pass = args.plugin_passphrase.clone().unwrap_or_default();
    tx.send(PluginStreamRequest {
        payload: Some(ReqPayload::Hello(Hello {
            plugin_name: "RemoteClient".into(),
            passphrase: plugin_pass,
        })),
    }).await.map_err(|_| Error::Auth("Failed sending Hello request.".into()))?;

    // Periodically log a message
    loop {
        time::sleep(Duration::from_secs(10)).await;
        if tx.send(PluginStreamRequest {
            payload: Some(ReqPayload::LogMessage(LogMessage {
                text: "RemoteClient is alive!".to_string(),
            })),
        }).await.is_err() {
            error!("Failed to send to server (maybe disconnected).");
            break;
        }
    }

    Ok(())
}
