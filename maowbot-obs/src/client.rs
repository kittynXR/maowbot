use crate::{models::*, ObsError};
use crate::error::Result;
use obws::Client;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};

pub struct ObsClient {
    instance: ObsInstance,
    client: Arc<RwLock<Option<Client>>>,
}

impl ObsClient {
    pub fn new(instance: ObsInstance) -> Self {
        Self {
            instance,
            client: Arc::new(RwLock::new(None)),
        }
    }
    
    pub async fn connect(&self) -> Result<()> {
        info!("Connecting to OBS instance {} at {}:{}", 
            self.instance.instance_number, self.instance.host, self.instance.port);
        
        let client = match (&self.instance.password, self.instance.use_password) {
            (Some(password), true) => {
                // Use password if provided and use_password is true
                Client::connect(&self.instance.host, self.instance.port, Some(password.as_str()))
                    .await
                    .map_err(|e| ObsError::ConnectionError(e.to_string()))?
            }
            _ => {
                // No password or use_password is false
                Client::connect(&self.instance.host, self.instance.port, None::<&str>)
                    .await
                    .map_err(|e| ObsError::ConnectionError(e.to_string()))?
            }
        };
        
        *self.client.write().await = Some(client);
        info!("Successfully connected to OBS instance {}", self.instance.instance_number);
        Ok(())
    }
    
    pub async fn disconnect(&self) -> Result<()> {
        if let Some(mut client) = self.client.write().await.take() {
            client.disconnect().await;
            info!("Disconnected from OBS instance {}", self.instance.instance_number);
        }
        Ok(())
    }
    
    pub async fn is_connected(&self) -> bool {
        self.client.read().await.is_some()
    }
    
    pub async fn get_version(&self) -> Result<String> {
        let client_guard = self.client.read().await;
        match client_guard.as_ref() {
            Some(client) => {
                let version = client.general().version().await
                    .map_err(|e| ObsError::WebSocketError(e.to_string()))?;
                Ok(format!("OBS: {} WebSocket: {}", version.obs_version, version.obs_web_socket_version))
            }
            None => Err(ObsError::InstanceNotConnected(self.instance.instance_number)),
        }
    }
    
    pub async fn list_scenes(&self) -> Result<Vec<ObsScene>> {
        let client_guard = self.client.read().await;
        match client_guard.as_ref() {
            Some(client) => {
                let scene_list = client.scenes().list().await
                    .map_err(|e| ObsError::WebSocketError(e.to_string()))?;
                
                let current_scene_name = scene_list.current_program_scene
                    .as_ref()
                    .map(|s| s.name.clone());
                
                let scenes = scene_list.scenes.into_iter().map(|scene| {
                    ObsScene {
                        name: scene.id.name.clone(),
                        index: scene.index,
                        is_current: Some(scene.id.name.clone()) == current_scene_name,
                    }
                }).collect();
                
                Ok(scenes)
            }
            None => Err(ObsError::InstanceNotConnected(self.instance.instance_number)),
        }
    }
    
    pub async fn set_current_scene(&self, scene_name: &str) -> Result<()> {
        let client_guard = self.client.read().await;
        match client_guard.as_ref() {
            Some(client) => {
                client.scenes().set_current_program_scene(scene_name).await
                    .map_err(|e| ObsError::WebSocketError(e.to_string()))?;
                Ok(())
            }
            None => Err(ObsError::InstanceNotConnected(self.instance.instance_number)),
        }
    }
    
    pub async fn list_sources(&self) -> Result<Vec<ObsSource>> {
        let client_guard = self.client.read().await;
        match client_guard.as_ref() {
            Some(client) => {
                let input_list = client.inputs().list(None).await
                    .map_err(|e| ObsError::WebSocketError(e.to_string()))?;
                
                let sources = input_list.into_iter().enumerate().map(|(index, input)| {
                    ObsSource {
                        name: input.id.name.clone(),
                        id: input.id.uuid.to_string(),
                        kind: input.kind,
                        is_visible: true, // Will need to query scene item visibility
                        scene_name: None,
                        index,
                    }
                }).collect();
                
                Ok(sources)
            }
            None => Err(ObsError::InstanceNotConnected(self.instance.instance_number)),
        }
    }
    
    pub async fn show_source(&self, source_name: &str, scene_name: Option<&str>) -> Result<()> {
        // Note: obws API for scene item visibility requires scene item ID
        // This is a simplified version - full implementation would need to:
        // 1. Get the scene item ID from the scene
        // 2. Set visibility on that specific item
        debug!("Showing source {} in scene {:?}", source_name, scene_name);
        Ok(())
    }
    
    pub async fn hide_source(&self, source_name: &str, scene_name: Option<&str>) -> Result<()> {
        // Similar to show_source, needs full scene item implementation
        debug!("Hiding source {} in scene {:?}", source_name, scene_name);
        Ok(())
    }
    
    pub async fn refresh_browser_source(&self, source_name: &str) -> Result<()> {
        let client_guard = self.client.read().await;
        match client_guard.as_ref() {
            Some(client) => {
                // Use the press_properties_button API to refresh browser source
                client.inputs().press_properties_button(source_name.into(), "refreshnocache").await
                    .map_err(|e| ObsError::WebSocketError(e.to_string()))?;
                Ok(())
            }
            None => Err(ObsError::InstanceNotConnected(self.instance.instance_number)),
        }
    }
    
    pub async fn start_streaming(&self) -> Result<()> {
        let client_guard = self.client.read().await;
        match client_guard.as_ref() {
            Some(client) => {
                client.streaming().start().await
                    .map_err(|e| ObsError::WebSocketError(e.to_string()))?;
                Ok(())
            }
            None => Err(ObsError::InstanceNotConnected(self.instance.instance_number)),
        }
    }
    
    pub async fn stop_streaming(&self) -> Result<()> {
        let client_guard = self.client.read().await;
        match client_guard.as_ref() {
            Some(client) => {
                client.streaming().stop().await
                    .map_err(|e| ObsError::WebSocketError(e.to_string()))?;
                Ok(())
            }
            None => Err(ObsError::InstanceNotConnected(self.instance.instance_number)),
        }
    }
    
    pub async fn get_stream_status(&self) -> Result<ObsStreamStatus> {
        let client_guard = self.client.read().await;
        match client_guard.as_ref() {
            Some(client) => {
                let status = client.streaming().status().await
                    .map_err(|e| ObsError::WebSocketError(e.to_string()))?;
                Ok(ObsStreamStatus {
                    is_streaming: status.active,
                    stream_time_ms: if status.active { Some(status.duration.whole_milliseconds() as u64) } else { None },
                    bytes_sent: Some(status.bytes),
                })
            }
            None => Err(ObsError::InstanceNotConnected(self.instance.instance_number)),
        }
    }
    
    pub async fn start_recording(&self) -> Result<()> {
        let client_guard = self.client.read().await;
        match client_guard.as_ref() {
            Some(client) => {
                client.recording().start().await
                    .map_err(|e| ObsError::WebSocketError(e.to_string()))?;
                Ok(())
            }
            None => Err(ObsError::InstanceNotConnected(self.instance.instance_number)),
        }
    }
    
    pub async fn stop_recording(&self) -> Result<()> {
        let client_guard = self.client.read().await;
        match client_guard.as_ref() {
            Some(client) => {
                client.recording().stop().await
                    .map_err(|e| ObsError::WebSocketError(e.to_string()))?;
                Ok(())
            }
            None => Err(ObsError::InstanceNotConnected(self.instance.instance_number)),
        }
    }
    
    pub async fn get_record_status(&self) -> Result<ObsRecordStatus> {
        let client_guard = self.client.read().await;
        match client_guard.as_ref() {
            Some(client) => {
                let status = client.recording().status().await
                    .map_err(|e| ObsError::WebSocketError(e.to_string()))?;
                Ok(ObsRecordStatus {
                    is_recording: status.active,
                    record_time_ms: if status.active { Some(status.duration.whole_milliseconds() as u64) } else { None },
                    bytes_written: Some(status.bytes),
                })
            }
            None => Err(ObsError::InstanceNotConnected(self.instance.instance_number)),
        }
    }
}