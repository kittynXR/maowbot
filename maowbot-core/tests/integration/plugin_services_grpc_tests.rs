// tests/integration/plugin_services_grpc_tests.rs

use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::mpsc;
use futures_util::{FutureExt, StreamExt};
use tokio::net::TcpListener;
use tokio_stream::wrappers::TcpListenerStream;
use tonic::transport::Server;

use maowbot_core::eventbus::EventBus;
use maowbot_core::plugins::manager::{PluginManager, PluginServiceGrpc};
use maowbot_proto::plugs::{
    plugin_service_server::PluginServiceServer,
    plugin_service_client::PluginServiceClient,
    PluginStreamRequest,
    plugin_stream_request::Payload as ReqPayload,
    plugin_stream_response::Payload as RespPayload,
    Hello, LogMessage,
};

#[tokio::test]
async fn test_grpc_end_to_end_hello() -> Result<(), Box<dyn std::error::Error>> {
    let bus = Arc::new(EventBus::new());
    let mut mgr = PluginManager::new(Some("mypassword".into()));
    mgr.set_event_bus(bus.clone());
    mgr.subscribe_to_event_bus(bus.clone());

    let pm = Arc::new(mgr);

    let socket_addr: SocketAddr = "127.0.0.1:0".parse()?;
    let listener = TcpListener::bind(socket_addr).await?;
    let local_addr = listener.local_addr()?;
    let inbound = TcpListenerStream::new(listener);

    let service = PluginServiceGrpc { manager: pm.clone() };
    let server = Server::builder().add_service(PluginServiceServer::new(service));

    let server_handle = tokio::spawn(async move {
        if let Err(e) = server.serve_with_incoming(inbound).await {
            eprintln!("Test server error: {:?}", e);
        }
    });

    let local_addr_str = format!("http://{}", local_addr);
    let channel = tonic::transport::Channel::from_shared(local_addr_str)?
        .connect()
        .await?;
    let mut client = PluginServiceClient::new(channel);

    let (tx, rx) = mpsc::channel::<PluginStreamRequest>(10);
    let in_stream = tokio_stream::wrappers::ReceiverStream::new(rx);

    let mut outbound = client.start_session(in_stream).await?.into_inner();

    let hello_req = PluginStreamRequest {
        payload: Some(ReqPayload::Hello(Hello {
            plugin_name: "End2EndTest".to_string(),
            passphrase: "mypassword".to_string(),
        })),
    };
    tx.send(hello_req).await?;

    if let Some(Ok(resp)) = outbound.next().await {
        match resp.payload {
            Some(RespPayload::Welcome(w)) => {
                assert_eq!(w.bot_name, "MaowBot");
            }
            other => panic!("Expected WelcomeResponse, got {:?}", other),
        }
    } else {
        panic!("No response received from server after sending Hello");
    }

    let log_req = PluginStreamRequest {
        payload: Some(ReqPayload::LogMessage(LogMessage {
            text: "Hello logs".to_string(),
        })),
    };
    tx.send(log_req).await?;

    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    if let Some(Ok(msg)) = outbound.next().now_or_never().flatten() {
        panic!("Expected no direct response to LogMessage, got {:?}", msg);
    }

    bus.shutdown();
    server_handle.abort();
    Ok(())
}