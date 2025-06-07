use anyhow::Result;
use maowbot_proto::maowbot::services::{
    ConfigureInstanceRequest, ListInstancesRequest, GetInstanceStatusRequest,
    ListScenesRequest, SelectSceneRequest, select_scene_request,
    ListSourcesRequest, SelectSourceRequest, select_source_request,
    ShowSourceRequest, HideSourceRequest, RefreshBrowserSourceRequest,
    StartStreamRequest, StopStreamRequest, GetStreamStatusRequest,
    StartRecordingRequest, StopRecordingRequest, GetRecordingStatusRequest,
    ObsInstance, ObsScene, ObsSource,
};
use crate::GrpcClient;

pub struct ObsCommands;

#[derive(Debug)]
pub struct ObsCommandResult {
    pub success: bool,
    pub message: String,
}

#[derive(Debug)]
pub struct ObsStatus {
    pub is_connected: bool,
    pub version: Option<String>,
    pub is_streaming: bool,
    pub stream_time_ms: Option<u64>,
    pub is_recording: bool,
    pub record_time_ms: Option<u64>,
}

impl ObsCommands {
    pub async fn configure_instance(
        client: &GrpcClient,
        instance_number: u32,
        property: &str,
        value: &str,
    ) -> Result<ObsCommandResult> {
        // Get current instance config
        let instances = Self::list_instances(client).await?;
        let current = instances.iter()
            .find(|i| i.instance_number == instance_number)
            .ok_or_else(|| anyhow::anyhow!("Instance {} not found", instance_number))?;
        
        let mut request = ConfigureInstanceRequest {
            instance_number,
            host: current.host.clone(),
            port: current.port,
            use_ssl: current.use_ssl,
            password: None, // Will be set if updating password
            use_password: current.use_password,
        };
        
        match property {
            "ip" | "host" => request.host = value.to_string(),
            "port" => request.port = value.parse::<u32>()
                .map_err(|_| anyhow::anyhow!("Invalid port number"))?,
            "ssl" => request.use_ssl = match value {
                "on" | "true" | "yes" => true,
                "off" | "false" | "no" => false,
                _ => return Err(anyhow::anyhow!("SSL must be on/off")),
            },
            "password" => request.password = Some(value.to_string()),
            "use_password" => request.use_password = match value {
                "on" | "true" | "yes" => true,
                "off" | "false" | "no" => false,
                _ => return Err(anyhow::anyhow!("use_password must be true/false")),
            },
            _ => return Err(anyhow::anyhow!("Unknown property: {}", property)),
        }
        
        let mut obs_client = client.obs.clone();
        let response = obs_client.configure_instance(request).await?;
        let inner = response.into_inner();
        
        Ok(ObsCommandResult {
            success: inner.success,
            message: if inner.success {
                format!("Updated {} for instance {}", property, instance_number)
            } else {
                inner.error_message.unwrap_or_else(|| "Configuration failed".to_string())
            },
        })
    }
    
    pub async fn list_instances(client: &GrpcClient) -> Result<Vec<ObsInstance>> {
        let mut obs_client = client.obs.clone();
        let response = obs_client.list_instances(ListInstancesRequest {}).await?;
        Ok(response.into_inner().instances)
    }
    
    pub async fn list_scenes(client: &GrpcClient, instance_number: u32) -> Result<Vec<ObsScene>> {
        let mut obs_client = client.obs.clone();
        let response = obs_client.list_scenes(ListScenesRequest { instance_number }).await?;
        Ok(response.into_inner().scenes)
    }
    
    pub async fn list_sources(client: &GrpcClient, instance_number: u32) -> Result<Vec<ObsSource>> {
        let mut obs_client = client.obs.clone();
        let response = obs_client.list_sources(ListSourcesRequest { instance_number }).await?;
        Ok(response.into_inner().sources)
    }
    
    pub async fn select_scene(
        client: &GrpcClient,
        instance_number: u32,
        scene: &str,
    ) -> Result<ObsCommandResult> {
        let selector = if let Ok(index) = scene.parse::<u32>() {
            select_scene_request::Selector::SceneIndex(index - 1) // Convert to 0-based
        } else {
            select_scene_request::Selector::SceneName(scene.to_string())
        };
        
        let mut obs_client = client.obs.clone();
        let response = obs_client.select_scene(SelectSceneRequest {
            instance_number,
            selector: Some(selector),
        }).await?;
        let inner = response.into_inner();
        
        Ok(ObsCommandResult {
            success: inner.success,
            message: if inner.success {
                format!("Selected scene: {}", scene)
            } else {
                inner.error_message.unwrap_or_else(|| "Failed to select scene".to_string())
            },
        })
    }
    
    pub async fn select_source(
        client: &GrpcClient,
        instance_number: u32,
        source: &str,
    ) -> Result<ObsCommandResult> {
        let selector = if let Ok(index) = source.parse::<u32>() {
            select_source_request::Selector::SourceIndex(index - 1) // Convert to 0-based
        } else {
            select_source_request::Selector::SourceName(source.to_string())
        };
        
        let mut obs_client = client.obs.clone();
        let response = obs_client.select_source(SelectSourceRequest {
            instance_number,
            selector: Some(selector),
        }).await?;
        let inner = response.into_inner();
        
        Ok(ObsCommandResult {
            success: inner.success,
            message: if inner.success {
                format!("Selected source: {}", inner.selected_source)
            } else {
                inner.error_message.unwrap_or_else(|| "Failed to select source".to_string())
            },
        })
    }
    
    pub async fn show_source(
        client: &GrpcClient,
        instance_number: u32,
        source_name: Option<&str>,
    ) -> Result<ObsCommandResult> {
        let source = match source_name {
            Some(name) => name.to_string(),
            None => return Err(anyhow::anyhow!("Source name required")),
        };
        
        let mut obs_client = client.obs.clone();
        let response = obs_client.show_source(ShowSourceRequest {
            instance_number,
            source_name: source.clone(),
            scene_name: None,
        }).await?;
        let inner = response.into_inner();
        
        Ok(ObsCommandResult {
            success: inner.success,
            message: if inner.success {
                format!("Showing source: {}", source)
            } else {
                inner.error_message.unwrap_or_else(|| "Failed to show source".to_string())
            },
        })
    }
    
    pub async fn hide_source(
        client: &GrpcClient,
        instance_number: u32,
        source_name: Option<&str>,
    ) -> Result<ObsCommandResult> {
        let source = match source_name {
            Some(name) => name.to_string(),
            None => return Err(anyhow::anyhow!("Source name required")),
        };
        
        let mut obs_client = client.obs.clone();
        let response = obs_client.hide_source(HideSourceRequest {
            instance_number,
            source_name: source.clone(),
            scene_name: None,
        }).await?;
        let inner = response.into_inner();
        
        Ok(ObsCommandResult {
            success: inner.success,
            message: if inner.success {
                format!("Hiding source: {}", source)
            } else {
                inner.error_message.unwrap_or_else(|| "Failed to hide source".to_string())
            },
        })
    }
    
    pub async fn refresh_source(
        client: &GrpcClient,
        instance_number: u32,
        source_name: Option<&str>,
    ) -> Result<ObsCommandResult> {
        let source = match source_name {
            Some(name) => name.to_string(),
            None => return Err(anyhow::anyhow!("Source name required")),
        };
        
        let mut obs_client = client.obs.clone();
        let response = obs_client.refresh_browser_source(RefreshBrowserSourceRequest {
            instance_number,
            source_name: source.clone(),
        }).await?;
        let inner = response.into_inner();
        
        Ok(ObsCommandResult {
            success: inner.success,
            message: if inner.success {
                format!("Refreshed browser source: {}", source)
            } else {
                inner.error_message.unwrap_or_else(|| "Failed to refresh source".to_string())
            },
        })
    }
    
    pub async fn start_stream(client: &GrpcClient, instance_number: u32) -> Result<ObsCommandResult> {
        let mut obs_client = client.obs.clone();
        let response = obs_client.start_stream(StartStreamRequest { instance_number }).await?;
        let inner = response.into_inner();
        
        Ok(ObsCommandResult {
            success: inner.success,
            message: if inner.success {
                "Started streaming".to_string()
            } else {
                inner.error_message.unwrap_or_else(|| "Failed to start stream".to_string())
            },
        })
    }
    
    pub async fn stop_stream(client: &GrpcClient, instance_number: u32) -> Result<ObsCommandResult> {
        let mut obs_client = client.obs.clone();
        let response = obs_client.stop_stream(StopStreamRequest { instance_number }).await?;
        let inner = response.into_inner();
        
        Ok(ObsCommandResult {
            success: inner.success,
            message: if inner.success {
                "Stopped streaming".to_string()
            } else {
                inner.error_message.unwrap_or_else(|| "Failed to stop stream".to_string())
            },
        })
    }
    
    pub async fn start_recording(client: &GrpcClient, instance_number: u32) -> Result<ObsCommandResult> {
        let mut obs_client = client.obs.clone();
        let response = obs_client.start_recording(StartRecordingRequest { instance_number }).await?;
        let inner = response.into_inner();
        
        Ok(ObsCommandResult {
            success: inner.success,
            message: if inner.success {
                "Started recording".to_string()
            } else {
                inner.error_message.unwrap_or_else(|| "Failed to start recording".to_string())
            },
        })
    }
    
    pub async fn stop_recording(client: &GrpcClient, instance_number: u32) -> Result<ObsCommandResult> {
        let mut obs_client = client.obs.clone();
        let response = obs_client.stop_recording(StopRecordingRequest { instance_number }).await?;
        let inner = response.into_inner();
        
        Ok(ObsCommandResult {
            success: inner.success,
            message: if inner.success {
                "Stopped recording".to_string()
            } else {
                inner.error_message.unwrap_or_else(|| "Failed to stop recording".to_string())
            },
        })
    }
    
    pub async fn get_status(client: &GrpcClient, instance_number: u32) -> Result<ObsStatus> {
        let mut obs_client = client.obs.clone();
        
        // Get connection status
        let status_response = obs_client.get_instance_status(GetInstanceStatusRequest { instance_number }).await?;
        let status_inner = status_response.into_inner();
        
        let mut status = ObsStatus {
            is_connected: status_inner.is_connected,
            version: status_inner.version,
            is_streaming: false,
            stream_time_ms: None,
            is_recording: false,
            record_time_ms: None,
        };
        
        // If connected, get stream and recording status
        if status.is_connected {
            // Get stream status
            if let Ok(stream_response) = obs_client.get_stream_status(GetStreamStatusRequest { instance_number }).await {
                let stream_inner = stream_response.into_inner();
                status.is_streaming = stream_inner.is_streaming;
                status.stream_time_ms = stream_inner.stream_time_ms;
            }
            
            // Get recording status
            if let Ok(record_response) = obs_client.get_recording_status(GetRecordingStatusRequest { instance_number }).await {
                let record_inner = record_response.into_inner();
                status.is_recording = record_inner.is_recording;
                status.record_time_ms = record_inner.record_time_ms;
            }
        }
        
        Ok(status)
    }
}