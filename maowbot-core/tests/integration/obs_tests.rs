use maowbot_core::test_utils::helpers::{setup_test_server, TestContext};
use maowbot_proto::maowbot::services::{
    ListObsInstancesRequest, UpdateObsInstanceRequest, ConnectObsInstanceRequest,
    DisconnectObsInstanceRequest, GetObsVersionRequest, ListObsScenesRequest,
    SelectObsSceneRequest, ListObsSourcesRequest, StartObsStreamRequest,
    StopObsStreamRequest, StartObsRecordRequest, StopObsRecordRequest,
    GetObsStatusRequest,
};

#[tokio::test]
async fn test_list_obs_instances() {
    let (server, _shutdown_tx) = setup_test_server().await;
    let mut obs_client = server.create_obs_client();
    
    let request = ListObsInstancesRequest {};
    let response = obs_client.list_obs_instances(request).await.unwrap();
    let instances = response.into_inner().instances;
    
    // Should have 2 default instances
    assert_eq!(instances.len(), 2);
    assert_eq!(instances[0].instance_number, 1);
    assert_eq!(instances[0].host, "127.0.0.1");
    assert_eq!(instances[0].port, 4455);
    assert_eq!(instances[1].instance_number, 2);
    assert_eq!(instances[1].host, "10.11.11.111");
}

#[tokio::test]
async fn test_update_obs_instance() {
    let (server, _shutdown_tx) = setup_test_server().await;
    let mut obs_client = server.create_obs_client();
    
    // Update instance 1
    let update_request = UpdateObsInstanceRequest {
        instance_number: 1,
        host: Some("192.168.1.100".to_string()),
        port: Some(4456),
        use_ssl: Some(true),
        password: Some("test123".to_string()),
    };
    
    let response = obs_client.update_obs_instance(update_request).await.unwrap();
    let updated = response.into_inner().instance.unwrap();
    
    assert_eq!(updated.host, "192.168.1.100");
    assert_eq!(updated.port, 4456);
    assert_eq!(updated.use_ssl, true);
    
    // Verify the update persisted
    let list_request = ListObsInstancesRequest {};
    let response = obs_client.list_obs_instances(list_request).await.unwrap();
    let instances = response.into_inner().instances;
    
    let instance = instances.iter().find(|i| i.instance_number == 1).unwrap();
    assert_eq!(instance.host, "192.168.1.100");
    assert_eq!(instance.port, 4456);
}

#[tokio::test]
async fn test_obs_connection_lifecycle() {
    let (server, _shutdown_tx) = setup_test_server().await;
    let mut obs_client = server.create_obs_client();
    
    // List instances first
    let list_request = ListObsInstancesRequest {};
    let response = obs_client.list_obs_instances(list_request).await.unwrap();
    let instances = response.into_inner().instances;
    
    // Verify initial state - not connected
    assert!(!instances[0].is_connected);
    
    // Note: Actual connection would fail without a real OBS instance
    // This test verifies the API structure works correctly
}

#[tokio::test]
async fn test_obs_version_requires_connection() {
    let (server, _shutdown_tx) = setup_test_server().await;
    let mut obs_client = server.create_obs_client();
    
    // Try to get version without connection
    let request = GetObsVersionRequest {
        instance_number: 1,
    };
    
    let result = obs_client.get_obs_version(request).await;
    
    // Should fail because not connected
    assert!(result.is_err());
    let error = result.unwrap_err();
    assert!(error.message().contains("not connected") || error.message().contains("Not connected"));
}

#[tokio::test]
async fn test_obs_scenes_requires_connection() {
    let (server, _shutdown_tx) = setup_test_server().await;
    let mut obs_client = server.create_obs_client();
    
    // Try to list scenes without connection
    let request = ListObsScenesRequest {
        instance_number: 1,
    };
    
    let result = obs_client.list_obs_scenes(request).await;
    
    // Should fail because not connected
    assert!(result.is_err());
}

#[tokio::test]
async fn test_obs_stream_control_requires_connection() {
    let (server, _shutdown_tx) = setup_test_server().await;
    let mut obs_client = server.create_obs_client();
    
    // Try to start stream without connection
    let start_request = StartObsStreamRequest {
        instance_number: 1,
    };
    
    let result = obs_client.start_obs_stream(start_request).await;
    assert!(result.is_err());
    
    // Try to stop stream without connection
    let stop_request = StopObsStreamRequest {
        instance_number: 1,
    };
    
    let result = obs_client.stop_obs_stream(stop_request).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_obs_record_control_requires_connection() {
    let (server, _shutdown_tx) = setup_test_server().await;
    let mut obs_client = server.create_obs_client();
    
    // Try to start recording without connection
    let start_request = StartObsRecordRequest {
        instance_number: 1,
    };
    
    let result = obs_client.start_obs_record(start_request).await;
    assert!(result.is_err());
    
    // Try to stop recording without connection
    let stop_request = StopObsRecordRequest {
        instance_number: 1,
    };
    
    let result = obs_client.stop_obs_record(stop_request).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_obs_status_requires_connection() {
    let (server, _shutdown_tx) = setup_test_server().await;
    let mut obs_client = server.create_obs_client();
    
    // Try to get status without connection
    let request = GetObsStatusRequest {
        instance_number: 1,
    };
    
    let result = obs_client.get_obs_status(request).await;
    
    // Should fail because not connected
    assert!(result.is_err());
}

#[tokio::test]
async fn test_invalid_instance_number() {
    let (server, _shutdown_tx) = setup_test_server().await;
    let mut obs_client = server.create_obs_client();
    
    // Try to connect to non-existent instance
    let request = ConnectObsInstanceRequest {
        instance_number: 999,
    };
    
    let result = obs_client.connect_obs_instance(request).await;
    assert!(result.is_err());
    let error = result.unwrap_err();
    assert!(error.message().contains("not found") || error.message().contains("Invalid"));
}