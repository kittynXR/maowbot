use crate::{AppEvent, ChatEvent};
use anyhow::Result;
use crossbeam_channel::{Receiver, Sender};
use tokio::sync::mpsc::unbounded_channel;
use tokio_stream::wrappers::UnboundedReceiverStream;
use tonic::transport::{Certificate, ClientTlsConfig, Endpoint};

use maowbot_proto::plugs::{
    plugin_service_client::PluginServiceClient,
    plugin_stream_request::Payload as ReqPayload,
    plugin_stream_response::Payload as RespPayload,
    Hello, PluginCapability, PluginStreamRequest, SendChat,
};
use crate::events::ChatCommand;

pub struct SharedGrpcClient;

impl SharedGrpcClient {
    pub fn start(
        plugin_name: String,
        event_tx: Sender<AppEvent>,
        command_rx: Receiver<ChatCommand>,
    ) {
        let url = std::env::var("MAOWBOT_GRPC_URL")
            .unwrap_or_else(|_| "https://localhost:9999".into());
        let token = std::env::var("MAOWBOT_GRPC_PASSPHRASE").unwrap_or_default();
        let ca_path = std::env::var("MAOWBOT_GRPC_CA")
            .unwrap_or_else(|_| "certs/server.crt".into());

        tokio::spawn(async move {
            loop {
                match Self::connect_and_run(
                    &url,
                    &token,
                    &ca_path,
                    &plugin_name,
                    event_tx.clone(),
                    command_rx.clone(),
                ).await {
                    Ok(_) => {
                        tracing::info!("gRPC connection closed normally");
                    }
                    Err(e) => {
                        tracing::error!("gRPC connection error: {}", e);
                        let _ = event_tx.send(AppEvent::GrpcStatusChanged(false));
                    }
                }

                // Wait before reconnecting
                tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
            }
        });
    }

    async fn connect_and_run(
        url: &str,
        token: &str,
        ca_path: &str,
        plugin_name: &str,
        event_tx: Sender<AppEvent>,
        command_rx: Receiver<ChatCommand>,
    ) -> Result<()> {
        let mut endpoint = Endpoint::new(url.to_string())?;

        if url.starts_with("https://") {
            let ca = tokio::fs::read(ca_path).await?;
            let ca_cert = Certificate::from_pem(ca);
            let tls = ClientTlsConfig::new()
                .ca_certificate(ca_cert)
                .domain_name("localhost");
            endpoint = endpoint.tls_config(tls)?;
        }

        let channel = endpoint.connect().await?;
        let mut client = PluginServiceClient::new(channel);

        let (tx_out, rx_out) = unbounded_channel::<PluginStreamRequest>();
        let outbound = UnboundedReceiverStream::new(rx_out);
        let response = client.start_session(outbound).await?;
        let mut inbound = response.into_inner();

        // Send Hello
        tx_out.send(PluginStreamRequest {
            payload: Some(ReqPayload::Hello(Hello {
                plugin_name: plugin_name.to_string(),
                passphrase: token.to_string(),
            })),
        })?;

        // Request capabilities
        tx_out.send(PluginStreamRequest {
            payload: Some(ReqPayload::RequestCaps(
                maowbot_proto::plugs::RequestCaps {
                    requested: vec![
                        PluginCapability::ReceiveChatEvents as i32,
                        PluginCapability::SendChat as i32,
                    ],
                },
            )),
        })?;

        let _ = event_tx.send(AppEvent::GrpcStatusChanged(true));

        // Spawn command handler
        let tx_out_clone = tx_out.clone();
        tokio::spawn(async move {
            while let Ok(cmd) = command_rx.recv() {
                match cmd {
                    ChatCommand::SendMessage(text) => {
                        let _ = tx_out_clone.send(PluginStreamRequest {
                            payload: Some(ReqPayload::SendChat(SendChat {
                                channel: "twitch".into(),
                                text,
                            })),
                        });
                    }
                }
            }
        });

        // Message pump
        while let Ok(Some(msg)) = inbound.message().await {
            if let Some(RespPayload::ChatMessage(cm)) = msg.payload {
                let _ = event_tx.send(AppEvent::Chat(ChatEvent {
                    channel: cm.channel,
                    author: cm.user,
                    body: cm.text,
                }));
            }
        }

        Ok(())
    }
}