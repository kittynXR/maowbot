use tonic::{Request, Response, Status};
use maowbot_proto::maowbot::services::{
    obs_service_server::ObsService,
    ConfigureInstanceRequest, ConfigureInstanceResponse,
    ListInstancesRequest, ListInstancesResponse, ObsInstance,
    GetInstanceStatusRequest, GetInstanceStatusResponse,
    ListScenesRequest, ListScenesResponse, ObsScene,
    SelectSceneRequest, SelectSceneResponse,
    ListSourcesRequest, ListSourcesResponse, ObsSource,
    SelectSourceRequest, SelectSourceResponse,
    ShowSourceRequest, ShowSourceResponse,
    HideSourceRequest, HideSourceResponse,
    RefreshBrowserSourceRequest, RefreshBrowserSourceResponse,
    StartStreamRequest, StartStreamResponse,
    StopStreamRequest, StopStreamResponse,
    GetStreamStatusRequest, GetStreamStatusResponse,
    StartRecordingRequest, StartRecordingResponse,
    StopRecordingRequest, StopRecordingResponse,
    GetRecordingStatusRequest, GetRecordingStatusResponse,
};
use maowbot_core::platforms::manager::PlatformManager;
use maowbot_core::repositories::postgres::obs::PostgresObsRepository;
use maowbot_common::traits::repository_traits::ObsRepository;
use std::sync::Arc;
use tracing::{info, error};

pub struct ObsServiceImpl {
    platform_manager: Arc<PlatformManager>,
    obs_repo: Arc<PostgresObsRepository>,
}

impl ObsServiceImpl {
    pub fn new(platform_manager: Arc<PlatformManager>, obs_repo: Arc<PostgresObsRepository>) -> Self {
        Self {
            platform_manager,
            obs_repo,
        }
    }
}

#[tonic::async_trait]
impl ObsService for ObsServiceImpl {
    async fn configure_instance(
        &self,
        request: Request<ConfigureInstanceRequest>,
    ) -> Result<Response<ConfigureInstanceResponse>, Status> {
        let req = request.into_inner();
        
        let instance = maowbot_obs::ObsInstance {
            instance_number: req.instance_number,
            host: req.host,
            port: req.port as u16,
            use_ssl: req.use_ssl,
            password: req.password,
            use_password: req.use_password,
        };
        
        match self.obs_repo.update_instance(&instance).await {
            Ok(_) => {
                info!("Updated OBS instance {} configuration", req.instance_number);
                Ok(Response::new(ConfigureInstanceResponse {
                    success: true,
                    error_message: None,
                }))
            }
            Err(e) => {
                error!("Failed to update OBS instance configuration: {}", e);
                Ok(Response::new(ConfigureInstanceResponse {
                    success: false,
                    error_message: Some(e.to_string()),
                }))
            }
        }
    }
    
    async fn list_instances(
        &self,
        _request: Request<ListInstancesRequest>,
    ) -> Result<Response<ListInstancesResponse>, Status> {
        match self.obs_repo.list_instances().await {
            Ok(instances) => {
                let proto_instances = instances.into_iter().map(|inst| {
                    ObsInstance {
                        instance_number: inst.instance_number,
                        host: inst.host,
                        port: inst.port as u32,
                        use_ssl: inst.use_ssl,
                        has_password: inst.password.is_some(),
                        is_connected: false, // TODO: Check actual connection status
                        last_connected_at: None,
                        use_password: inst.use_password,
                    }
                }).collect();
                
                Ok(Response::new(ListInstancesResponse {
                    instances: proto_instances,
                }))
            }
            Err(e) => {
                error!("Failed to list OBS instances: {}", e);
                Err(Status::internal(e.to_string()))
            }
        }
    }
    
    async fn get_instance_status(
        &self,
        request: Request<GetInstanceStatusRequest>,
    ) -> Result<Response<GetInstanceStatusResponse>, Status> {
        let instance_number = request.into_inner().instance_number;
        
        match self.platform_manager.get_obs_instance(instance_number).await {
            Ok(obs_runtime) => {
                let client = obs_runtime.get_client();
                let is_connected = client.is_connected().await;
                let version = if is_connected {
                    client.get_version().await.ok()
                } else {
                    None
                };
                
                Ok(Response::new(GetInstanceStatusResponse {
                    is_connected,
                    version,
                    error_message: None,
                }))
            }
            Err(e) => {
                Ok(Response::new(GetInstanceStatusResponse {
                    is_connected: false,
                    version: None,
                    error_message: Some(e.to_string()),
                }))
            }
        }
    }
    
    async fn list_scenes(
        &self,
        request: Request<ListScenesRequest>,
    ) -> Result<Response<ListScenesResponse>, Status> {
        let instance_number = request.into_inner().instance_number;
        
        let obs_runtime = self.platform_manager.get_obs_instance(instance_number).await
            .map_err(|e| Status::internal(e.to_string()))?;
        
        let client = obs_runtime.get_client();
        match client.list_scenes().await {
            Ok(scenes) => {
                let proto_scenes = scenes.into_iter().map(|scene| {
                    ObsScene {
                        name: scene.name,
                        index: scene.index as u32,
                        is_current: scene.is_current,
                    }
                }).collect();
                
                Ok(Response::new(ListScenesResponse {
                    scenes: proto_scenes,
                }))
            }
            Err(e) => {
                error!("Failed to list scenes: {}", e);
                Err(Status::internal(e.to_string()))
            }
        }
    }
    
    async fn select_scene(
        &self,
        request: Request<SelectSceneRequest>,
    ) -> Result<Response<SelectSceneResponse>, Status> {
        let req = request.into_inner();
        let instance_number = req.instance_number;
        
        let obs_runtime = self.platform_manager.get_obs_instance(instance_number).await
            .map_err(|e| Status::internal(e.to_string()))?;
        
        let client = obs_runtime.get_client();
        
        // Determine scene name based on selector
        use maowbot_proto::maowbot::services::select_scene_request;
        
        let scene_name = match req.selector {
            Some(select_scene_request::Selector::SceneName(name)) => name,
            Some(select_scene_request::Selector::SceneIndex(index)) => {
                // Get scene list to find name by index
                match client.list_scenes().await {
                    Ok(scenes) => {
                        scenes.get(index as usize)
                            .map(|s| s.name.clone())
                            .ok_or_else(|| Status::invalid_argument(format!("Scene index {} out of bounds", index)))?
                    }
                    Err(e) => return Err(Status::internal(e.to_string())),
                }
            }
            None => return Err(Status::invalid_argument("No scene selector provided")),
        };
        
        match client.set_current_scene(&scene_name).await {
            Ok(_) => {
                Ok(Response::new(SelectSceneResponse {
                    success: true,
                    error_message: None,
                }))
            }
            Err(e) => {
                Ok(Response::new(SelectSceneResponse {
                    success: false,
                    error_message: Some(e.to_string()),
                }))
            }
        }
    }
    
    async fn list_sources(
        &self,
        request: Request<ListSourcesRequest>,
    ) -> Result<Response<ListSourcesResponse>, Status> {
        let instance_number = request.into_inner().instance_number;
        
        let obs_runtime = self.platform_manager.get_obs_instance(instance_number).await
            .map_err(|e| Status::internal(e.to_string()))?;
        
        let client = obs_runtime.get_client();
        match client.list_sources().await {
            Ok(sources) => {
                let proto_sources = sources.into_iter().map(|source| {
                    ObsSource {
                        name: source.name,
                        id: source.id,
                        kind: source.kind,
                        is_visible: source.is_visible,
                        scene_name: source.scene_name,
                        index: source.index as u32,
                    }
                }).collect();
                
                Ok(Response::new(ListSourcesResponse {
                    sources: proto_sources,
                }))
            }
            Err(e) => {
                error!("Failed to list sources: {}", e);
                Err(Status::internal(e.to_string()))
            }
        }
    }
    
    async fn select_source(
        &self,
        request: Request<SelectSourceRequest>,
    ) -> Result<Response<SelectSourceResponse>, Status> {
        let req = request.into_inner();
        
        // Source selection is a UI concept for the TUI
        // We just validate that the source exists
        use maowbot_proto::maowbot::services::select_source_request;
        
        let source_name = match req.selector {
            Some(select_source_request::Selector::SourceName(name)) => name,
            Some(select_source_request::Selector::SourceIndex(index)) => {
                // Get source list to find name by index
                let obs_runtime = self.platform_manager.get_obs_instance(req.instance_number).await
                    .map_err(|e| Status::internal(e.to_string()))?;
                
                let client = obs_runtime.get_client();
                match client.list_sources().await {
                    Ok(sources) => {
                        sources.get(index as usize)
                            .map(|s| s.name.clone())
                            .ok_or_else(|| Status::invalid_argument(format!("Source index {} out of bounds", index)))?
                    }
                    Err(e) => return Err(Status::internal(e.to_string())),
                }
            }
            None => return Err(Status::invalid_argument("No source selector provided")),
        };
        
        Ok(Response::new(SelectSourceResponse {
            success: true,
            selected_source: source_name,
            error_message: None,
        }))
    }
    
    async fn show_source(
        &self,
        request: Request<ShowSourceRequest>,
    ) -> Result<Response<ShowSourceResponse>, Status> {
        let req = request.into_inner();
        
        let obs_runtime = self.platform_manager.get_obs_instance(req.instance_number).await
            .map_err(|e| Status::internal(e.to_string()))?;
        
        let client = obs_runtime.get_client();
        match client.show_source(&req.source_name, req.scene_name.as_deref()).await {
            Ok(_) => {
                Ok(Response::new(ShowSourceResponse {
                    success: true,
                    error_message: None,
                }))
            }
            Err(e) => {
                Ok(Response::new(ShowSourceResponse {
                    success: false,
                    error_message: Some(e.to_string()),
                }))
            }
        }
    }
    
    async fn hide_source(
        &self,
        request: Request<HideSourceRequest>,
    ) -> Result<Response<HideSourceResponse>, Status> {
        let req = request.into_inner();
        
        let obs_runtime = self.platform_manager.get_obs_instance(req.instance_number).await
            .map_err(|e| Status::internal(e.to_string()))?;
        
        let client = obs_runtime.get_client();
        match client.hide_source(&req.source_name, req.scene_name.as_deref()).await {
            Ok(_) => {
                Ok(Response::new(HideSourceResponse {
                    success: true,
                    error_message: None,
                }))
            }
            Err(e) => {
                Ok(Response::new(HideSourceResponse {
                    success: false,
                    error_message: Some(e.to_string()),
                }))
            }
        }
    }
    
    async fn refresh_browser_source(
        &self,
        request: Request<RefreshBrowserSourceRequest>,
    ) -> Result<Response<RefreshBrowserSourceResponse>, Status> {
        let req = request.into_inner();
        
        let obs_runtime = self.platform_manager.get_obs_instance(req.instance_number).await
            .map_err(|e| Status::internal(e.to_string()))?;
        
        let client = obs_runtime.get_client();
        match client.refresh_browser_source(&req.source_name).await {
            Ok(_) => {
                Ok(Response::new(RefreshBrowserSourceResponse {
                    success: true,
                    error_message: None,
                }))
            }
            Err(e) => {
                Ok(Response::new(RefreshBrowserSourceResponse {
                    success: false,
                    error_message: Some(e.to_string()),
                }))
            }
        }
    }
    
    async fn start_stream(
        &self,
        request: Request<StartStreamRequest>,
    ) -> Result<Response<StartStreamResponse>, Status> {
        let instance_number = request.into_inner().instance_number;
        
        let obs_runtime = self.platform_manager.get_obs_instance(instance_number).await
            .map_err(|e| Status::internal(e.to_string()))?;
        
        let client = obs_runtime.get_client();
        match client.start_streaming().await {
            Ok(_) => {
                Ok(Response::new(StartStreamResponse {
                    success: true,
                    error_message: None,
                }))
            }
            Err(e) => {
                Ok(Response::new(StartStreamResponse {
                    success: false,
                    error_message: Some(e.to_string()),
                }))
            }
        }
    }
    
    async fn stop_stream(
        &self,
        request: Request<StopStreamRequest>,
    ) -> Result<Response<StopStreamResponse>, Status> {
        let instance_number = request.into_inner().instance_number;
        
        let obs_runtime = self.platform_manager.get_obs_instance(instance_number).await
            .map_err(|e| Status::internal(e.to_string()))?;
        
        let client = obs_runtime.get_client();
        match client.stop_streaming().await {
            Ok(_) => {
                Ok(Response::new(StopStreamResponse {
                    success: true,
                    error_message: None,
                }))
            }
            Err(e) => {
                Ok(Response::new(StopStreamResponse {
                    success: false,
                    error_message: Some(e.to_string()),
                }))
            }
        }
    }
    
    async fn get_stream_status(
        &self,
        request: Request<GetStreamStatusRequest>,
    ) -> Result<Response<GetStreamStatusResponse>, Status> {
        let instance_number = request.into_inner().instance_number;
        
        let obs_runtime = self.platform_manager.get_obs_instance(instance_number).await
            .map_err(|e| Status::internal(e.to_string()))?;
        
        let client = obs_runtime.get_client();
        match client.get_stream_status().await {
            Ok(status) => {
                Ok(Response::new(GetStreamStatusResponse {
                    is_streaming: status.is_streaming,
                    stream_time_ms: status.stream_time_ms,
                    bytes_sent: status.bytes_sent,
                }))
            }
            Err(e) => {
                error!("Failed to get stream status: {}", e);
                Err(Status::internal(e.to_string()))
            }
        }
    }
    
    async fn start_recording(
        &self,
        request: Request<StartRecordingRequest>,
    ) -> Result<Response<StartRecordingResponse>, Status> {
        let instance_number = request.into_inner().instance_number;
        
        let obs_runtime = self.platform_manager.get_obs_instance(instance_number).await
            .map_err(|e| Status::internal(e.to_string()))?;
        
        let client = obs_runtime.get_client();
        match client.start_recording().await {
            Ok(_) => {
                Ok(Response::new(StartRecordingResponse {
                    success: true,
                    error_message: None,
                }))
            }
            Err(e) => {
                Ok(Response::new(StartRecordingResponse {
                    success: false,
                    error_message: Some(e.to_string()),
                }))
            }
        }
    }
    
    async fn stop_recording(
        &self,
        request: Request<StopRecordingRequest>,
    ) -> Result<Response<StopRecordingResponse>, Status> {
        let instance_number = request.into_inner().instance_number;
        
        let obs_runtime = self.platform_manager.get_obs_instance(instance_number).await
            .map_err(|e| Status::internal(e.to_string()))?;
        
        let client = obs_runtime.get_client();
        match client.stop_recording().await {
            Ok(_) => {
                Ok(Response::new(StopRecordingResponse {
                    success: true,
                    error_message: None,
                }))
            }
            Err(e) => {
                Ok(Response::new(StopRecordingResponse {
                    success: false,
                    error_message: Some(e.to_string()),
                }))
            }
        }
    }
    
    async fn get_recording_status(
        &self,
        request: Request<GetRecordingStatusRequest>,
    ) -> Result<Response<GetRecordingStatusResponse>, Status> {
        let instance_number = request.into_inner().instance_number;
        
        let obs_runtime = self.platform_manager.get_obs_instance(instance_number).await
            .map_err(|e| Status::internal(e.to_string()))?;
        
        let client = obs_runtime.get_client();
        match client.get_record_status().await {
            Ok(status) => {
                Ok(Response::new(GetRecordingStatusResponse {
                    is_recording: status.is_recording,
                    record_time_ms: status.record_time_ms,
                    bytes_written: status.bytes_written,
                }))
            }
            Err(e) => {
                error!("Failed to get recording status: {}", e);
                Err(Status::internal(e.to_string()))
            }
        }
    }
}