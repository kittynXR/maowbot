use bevy::prelude::*;
use maowbot_proto::plugs::{plugin_service_client::PluginServiceClient, plugin_stream_request::{Payload as ReqPayload}, plugin_stream_response::{Payload as RespPayload}, PluginStreamRequest, PluginStreamResponse, Hello, SendChat};
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
use tokio_stream::wrappers::UnboundedReceiverStream;
use tonic::transport::{Certificate, Channel, ClientTlsConfig, Endpoint};
use crate::chat_hud::{ChatSendEvent};

pub struct OverlayGrpcPlugin;

#[derive(Resource)]
struct Tx(UnboundedSender<PluginStreamRequest>);
#[derive(Resource)]
struct Rx(UnboundedReceiver<PluginStreamResponse>);

#[derive(Event)]
pub struct ChatEvent {
    pub channel: String,
    pub author:  String,
    pub body:    String,
}

impl Plugin for OverlayGrpcPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<ChatEvent>()
            .add_systems(Startup, setup_ipc)
            .add_systems(Update, (forward_inbound, send_chat_to_core));
    }
}

/// Spawn a Tokio task that connects to the plugin server and pumps messages.
fn setup_ipc(mut cmds: Commands) {
    let (tx_out, rx_out) = unbounded_channel::<PluginStreamRequest>();
    let (tx_in,  rx_in)  = unbounded_channel::<PluginStreamResponse>();

    cmds.insert_resource(Tx(tx_out.clone()));
    cmds.insert_resource(Rx(rx_in));

    let url = std::env::var("MAOWBOT_GRPC_URL").unwrap_or_else(|_| "https://localhost:9999".into());
    let token = std::env::var("MAOWBOT_GRPC_PASSPHRASE").unwrap_or_default();
    let ca_path = std::env::var("MAOWBOT_GRPC_CA").unwrap_or_else(|_| "certs/server.crt".into());

    tokio::spawn(async move {
        let mut endpoint = Endpoint::new(url.clone()).expect("Invalid gRPC URL");

        if url.starts_with("https://") {
            let ca = tokio::fs::read(&ca_path).await.expect("Failed to read CA");
            let ca_cert = Certificate::from_pem(ca);
            let tls = ClientTlsConfig::new()
                .ca_certificate(ca_cert)
                .domain_name("localhost");
            endpoint = endpoint.tls_config(tls).expect("Invalid TLS config");
        }

        let channel = endpoint.connect().await.expect("gRPC connection failed");
        let mut client = PluginServiceClient::new(channel);

        let outbound = UnboundedReceiverStream::new(rx_out);
        let response = client.start_session(outbound)
            .await.expect("start_session failed");
        let mut inbound = response.into_inner();

        // Send Hello message AFTER the stream is open
        let _ = tx_out.send(PluginStreamRequest {
            payload: Some(ReqPayload::Hello(Hello {
                plugin_name: "maowbot-gui".into(),
                passphrase: token,
            })),
        });

        // ---- NEW: ask for capabilities we need ----
        use maowbot_proto::plugs::{PluginCapability, plugin_stream_request::Payload::RequestCaps};
        let _ = tx_out.send(PluginStreamRequest {
            payload: Some(RequestCaps(maowbot_proto::plugs::RequestCaps {
                requested: vec![
                    PluginCapability::ReceiveChatEvents as i32,
                    PluginCapability::SendChat as i32,
                ],
            })),
        });
        
        while let Ok(Some(msg)) = inbound.message().await {
            let _ = tx_in.send(msg);
        }

        tracing::warn!("PluginService stream closed");
    });
}

/// Forward ChatMessage responses to Bevy event system.
fn forward_inbound(mut rx: ResMut<Rx>, mut writer: EventWriter<ChatEvent>) {
    while let Ok(msg) = rx.0.try_recv() {
        if let Some(RespPayload::ChatMessage(cm)) = msg.payload {
            writer.write(ChatEvent {
                channel: cm.channel,
                author:  cm.user,
                body:    cm.text,
            });
        }
    }
}

fn send_chat_to_core(
    mut ev: EventReader<ChatSendEvent>,
    tx: Res<Tx>,
) {
    for ChatSendEvent { text } in ev.read() {
        let _ = tx.0.send(PluginStreamRequest {
            payload: Some(ReqPayload::SendChat(SendChat {
                channel: "twitch".into(),
                text:    text.clone(),   // ← send the actual text
            })),
        });
    }
}