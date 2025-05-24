use crossbeam_channel::{Receiver, Sender};
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc::unbounded_channel;
use tokio_stream::wrappers::UnboundedReceiverStream;
use tonic::transport::{Certificate, ClientTlsConfig, Endpoint};

use crate::chat::{ChatCommand, ChatEvent, ChatState};
use crate::AppEvent;

use maowbot_proto::plugs::{
    plugin_service_client::PluginServiceClient,
    plugin_stream_request::{Payload as ReqPayload},
    plugin_stream_response::{Payload as RespPayload},
    Hello, PluginCapability, PluginStreamRequest, SendChat,
};

pub fn start_grpc_client(
    event_tx: Sender<AppEvent>,
    command_rx: Receiver<ChatCommand>,
    _chat_state: Arc<Mutex<ChatState>>,
) {
    let url = std::env::var("MAOWBOT_GRPC_URL")
        .unwrap_or_else(|_| "https://localhost:9999".into());
    let token = std::env::var("MAOWBOT_GRPC_PASSPHRASE").unwrap_or_default();
    let ca_path = std::env::var("MAOWBOT_GRPC_CA")
        .unwrap_or_else(|_| "certs/server.crt".into());

    tokio::spawn(async move {
        let mut endpoint = Endpoint::new(url.clone()).expect("Invalid gRPC URL");

        if url.starts_with("https://") {
            let ca = tokio::fs::read(&ca_path)
                .await
                .expect("Failed to read CA");
            let ca_cert = Certificate::from_pem(ca);
            let tls = ClientTlsConfig::new()
                .ca_certificate(ca_cert)
                .domain_name("localhost");
            endpoint = endpoint.tls_config(tls).expect("Invalid TLS config");
        }

        let channel = endpoint.connect().await.expect("gRPC connection failed");
        let mut client = PluginServiceClient::new(channel);

        let (tx_out, rx_out) = unbounded_channel::<PluginStreamRequest>();
        let outbound = UnboundedReceiverStream::new(rx_out);
        let response = client
            .start_session(outbound)
            .await
            .expect("start_session failed");
        let mut inbound = response.into_inner();

        // Send Hello
        let _ = tx_out.send(PluginStreamRequest {
            payload: Some(ReqPayload::Hello(Hello {
                plugin_name: "maowbot-overlay".into(),
                passphrase: token,
            })),
        });

        // Request capabilities
        let _ = tx_out.send(PluginStreamRequest {
            payload: Some(ReqPayload::RequestCaps(
                maowbot_proto::plugs::RequestCaps {
                    requested: vec![
                        PluginCapability::ReceiveChatEvents as i32,
                        PluginCapability::SendChat as i32,
                    ],
                },
            )),
        });

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

        tracing::warn!("PluginService stream closed");
    });
}