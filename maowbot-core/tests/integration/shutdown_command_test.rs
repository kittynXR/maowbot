use maowbot_core::eventbus::EventBus;
use maowbot_proto::maowbot::services::{
    config_service_server::{ConfigService, ConfigServiceServer},
    ShutdownServerRequest, ShutdownServerResponse,
};
use std::sync::Arc;
use tonic::{transport::Server, Request, Response, Status};
use tokio::sync::watch;

/// Test implementation of ConfigService that just handles shutdown
struct TestConfigService {
    event_bus: Arc<EventBus>,
}

#[tonic::async_trait]
impl ConfigService for TestConfigService {
    async fn shutdown_server(
        &self,
        request: Request<ShutdownServerRequest>,
    ) -> Result<Response<ShutdownServerResponse>, Status> {
        let req = request.into_inner();
        let grace_period = if req.grace_period_seconds > 0 {
            req.grace_period_seconds
        } else {
            30
        };

        // Schedule shutdown
        let event_bus = self.event_bus.clone();
        tokio::spawn(async move {
            tokio::time::sleep(tokio::time::Duration::from_secs(grace_period as u64)).await;
            event_bus.shutdown();
        });

        Ok(Response::new(ShutdownServerResponse {
            accepted: true,
            message: format!("Shutdown scheduled in {} seconds", grace_period),
            shutdown_at: None,
        }))
    }

    // Other methods would return unimplemented
    async fn get_config(
        &self,
        _: Request<maowbot_proto::maowbot::services::GetConfigRequest>,
    ) -> Result<Response<maowbot_proto::maowbot::services::GetConfigResponse>, Status> {
        Err(Status::unimplemented("Not implemented"))
    }

    async fn set_config(
        &self,
        _: Request<maowbot_proto::maowbot::services::SetConfigRequest>,
    ) -> Result<Response<maowbot_proto::maowbot::services::SetConfigResponse>, Status> {
        Err(Status::unimplemented("Not implemented"))
    }

    async fn delete_config(
        &self,
        _: Request<maowbot_proto::maowbot::services::DeleteConfigRequest>,
    ) -> Result<Response<()>, Status> {
        Err(Status::unimplemented("Not implemented"))
    }

    async fn list_configs(
        &self,
        _: Request<maowbot_proto::maowbot::services::ListConfigsRequest>,
    ) -> Result<Response<maowbot_proto::maowbot::services::ListConfigsResponse>, Status> {
        Err(Status::unimplemented("Not implemented"))
    }

    async fn batch_get_configs(
        &self,
        _: Request<maowbot_proto::maowbot::services::BatchGetConfigsRequest>,
    ) -> Result<Response<maowbot_proto::maowbot::services::BatchGetConfigsResponse>, Status> {
        Err(Status::unimplemented("Not implemented"))
    }

    async fn batch_set_configs(
        &self,
        _: Request<maowbot_proto::maowbot::services::BatchSetConfigsRequest>,
    ) -> Result<Response<maowbot_proto::maowbot::services::BatchSetConfigsResponse>, Status> {
        Err(Status::unimplemented("Not implemented"))
    }

    async fn validate_config(
        &self,
        _: Request<maowbot_proto::maowbot::services::ValidateConfigRequest>,
    ) -> Result<Response<maowbot_proto::maowbot::services::ValidateConfigResponse>, Status> {
        Err(Status::unimplemented("Not implemented"))
    }

    async fn get_config_history(
        &self,
        _: Request<maowbot_proto::maowbot::services::GetConfigHistoryRequest>,
    ) -> Result<Response<maowbot_proto::maowbot::services::GetConfigHistoryResponse>, Status> {
        Err(Status::unimplemented("Not implemented"))
    }

    async fn export_configs(
        &self,
        _: Request<maowbot_proto::maowbot::services::ExportConfigsRequest>,
    ) -> Result<Response<maowbot_proto::maowbot::services::ExportConfigsResponse>, Status> {
        Err(Status::unimplemented("Not implemented"))
    }

    async fn import_configs(
        &self,
        _: Request<maowbot_proto::maowbot::services::ImportConfigsRequest>,
    ) -> Result<Response<maowbot_proto::maowbot::services::ImportConfigsResponse>, Status> {
        Err(Status::unimplemented("Not implemented"))
    }

    type StreamConfigUpdatesStream = tonic::codec::Streaming<maowbot_proto::maowbot::services::ConfigUpdateEvent>;

    async fn stream_config_updates(
        &self,
        _: Request<maowbot_proto::maowbot::services::StreamConfigUpdatesRequest>,
    ) -> Result<Response<Self::StreamConfigUpdatesStream>, Status> {
        Err(Status::unimplemented("Not implemented"))
    }
}

#[tokio::test]
async fn test_shutdown_command() {
    let event_bus = Arc::new(EventBus::new());
    let config_service = TestConfigService {
        event_bus: event_bus.clone(),
    };

    // Start a test server
    let addr = "127.0.0.1:0".parse().unwrap();
    let server = Server::builder()
        .add_service(ConfigServiceServer::new(config_service))
        .serve(addr);

    let server_addr = server.local_addr();
    let server_handle = tokio::spawn(server);

    // Give server time to start
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Create a client and send shutdown request
    let url = format!("http://{}", server_addr);
    let mut client = maowbot_proto::maowbot::services::config_service_client::ConfigServiceClient::connect(url)
        .await
        .unwrap();

    let request = ShutdownServerRequest {
        reason: "test".to_string(),
        grace_period_seconds: 1, // Short grace period for testing
    };

    let response = client.shutdown_server(request).await.unwrap();
    let response = response.into_inner();

    assert!(response.accepted);
    assert!(response.message.contains("Shutdown scheduled"));

    // Wait for shutdown to occur
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    // Verify event bus is shut down
    assert!(event_bus.is_shutdown());

    // Clean up
    server_handle.abort();
}