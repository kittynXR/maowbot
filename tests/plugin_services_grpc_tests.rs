// tests/plugin_services_grpc_tests.rs

use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::mpsc;
use futures_util::{FutureExt, StreamExt};
use tokio::net::TcpListener;
use tokio::task::JoinHandle;
use tokio_stream::wrappers::TcpListenerStream;
use tonic::transport::Server;

use maowbot::eventbus::EventBus;
use maowbot::plugins::manager::{PluginManager, PluginServiceGrpc};
use maowbot_proto::plugs::{
    plugin_service_server::PluginServiceServer,
    plugin_service_client::PluginServiceClient,
    PluginStreamRequest,
    plugin_stream_request::Payload as ReqPayload,
    plugin_stream_response::Payload as RespPayload,
    Hello, LogMessage,
};

/// Spawns a gRPC server listening on an ephemeral port, returning the bind address + join handle.
async fn spawn_grpc_server(
    pm: Arc<PluginManager>
) -> Result<(String, JoinHandle<()>), Box<dyn std::error::Error>> {
    // 1) Use IPv4 on Windows for ephemeral port
    let socket_addr: SocketAddr = "127.0.0.1:0".parse()?;

    // 2) Bind the TcpListener
    let listener = TcpListener::bind(socket_addr).await?;
    let local_addr = listener.local_addr()?;

    // 3) Wrap the listener in a stream for Tonic
    let inbound = TcpListenerStream::new(listener);

    // 4) Build the Tonic server with our PluginServiceGrpc
    let service = PluginServiceGrpc { manager: pm };
    let server = Server::builder().add_service(PluginServiceServer::new(service));

    // 5) Spawn the server on a separate async task
    let handle = tokio::spawn(async move {
        if let Err(e) = server.serve_with_incoming(inbound).await {
            eprintln!("Test server error: {:?}", e);
        }
    });

    let local_addr_str = format!("http://{}", local_addr);
    Ok((local_addr_str, handle))
}

#[tokio::test]
async fn test_grpc_end_to_end_hello() -> Result<(), Box<dyn std::error::Error>> {
    // 1) Build an EventBus + PluginManager with passphrase = "mypassword"
    let bus = Arc::new(EventBus::new());
    let mut mgr = PluginManager::new(Some("mypassword".into()));
    mgr.set_event_bus(bus.clone());
    mgr.subscribe_to_event_bus(bus.clone());

    // Wrap it in Arc for concurrency
    let pm = Arc::new(mgr);

    // 2) Spawn our gRPC server
    let (server_url, server_handle) = spawn_grpc_server(pm.clone()).await?;

    // 3) Create a Tonic client
    let channel = tonic::transport::Channel::from_shared(server_url)?
        .connect()
        .await?;
    let mut client = PluginServiceClient::new(channel);

    // We'll send PluginStreamRequest items over an mpsc channel
    let (tx, rx) = mpsc::channel::<PluginStreamRequest>(10);
    let in_stream = tokio_stream::wrappers::ReceiverStream::new(rx);

    // Start session => get the server->client stream
    let mut outbound = client.start_session(in_stream).await?.into_inner();

    // 4) Send Hello with the correct passphrase
    let hello_req = PluginStreamRequest {
        payload: Some(ReqPayload::Hello(Hello {
            plugin_name: "End2EndTest".to_string(),
            passphrase: "mypassword".to_string(),
        })),
    };
    tx.send(hello_req).await?;

    // 5) Expect a WelcomeResponse
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

    // 6) Send a LogMessage => we do NOT expect any direct response
    let log_req = PluginStreamRequest {
        payload: Some(ReqPayload::LogMessage(LogMessage {
            text: "Hello logs".to_string(),
        })),
    };
    tx.send(log_req).await?;

    // Give the server a moment to process
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    // If there's any new message, that would be unexpected
    if let Some(Ok(msg)) = outbound.next().now_or_never().flatten() {
        panic!("Expected no direct response to LogMessage, got {:?}", msg);
    }

    // 7) Cleanup: shut down our event bus, then abort the server.
    bus.shutdown();
    server_handle.abort();

    Ok(())
}
