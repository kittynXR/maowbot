use tonic::{Request, Response, Status};
use maowbot_proto::maowbot::services::{
    plugin_service_server::PluginService as GrpcPluginService,
    *,
};
use maowbot_proto::maowbot::common::Plugin as ProtoPlugin;
use maowbot_core::plugins::manager::PluginManager;
use maowbot_common::traits::api::PluginApi;
use std::sync::Arc;
use std::collections::HashMap;
use tracing::{info, error, debug};
use prost_types;
use uuid;
use chrono::Utc;

pub struct PluginServiceImpl {
    plugin_manager: Arc<PluginManager>,
}

impl PluginServiceImpl {
    pub fn new(plugin_manager: Arc<PluginManager>) -> Self {
        Self { plugin_manager }
    }
}

#[tonic::async_trait]
impl GrpcPluginService for PluginServiceImpl {
    async fn list_plugins(
        &self,
        request: Request<ListPluginsRequest>,
    ) -> Result<Response<ListPluginsResponse>, Status> {
        let req = request.into_inner();
        debug!("Listing plugins - active_only: {}", req.active_only);
        
        // Get plugin records and connected plugin info
        let plugin_records = self.plugin_manager.get_plugin_records();
        let connected_plugins = self.plugin_manager.list_connected_plugins().await;
        
        let mut plugin_infos = Vec::new();
        
        for record in plugin_records {
            // Skip disabled plugins if active_only is true
            if req.active_only && !record.enabled {
                continue;
            }
            
            // Skip non-system plugins if include_system_plugins is false
            // (For now, we'll consider all plugins as non-system)
            
            // Find if this plugin is connected
            let connected_info = connected_plugins.iter()
                .find(|p| p.name == record.name);
            
            let state = if let Some(info) = connected_info {
                if info.is_enabled {
                    plugin_status::State::Running
                } else {
                    plugin_status::State::Loaded
                }
            } else if record.enabled {
                plugin_status::State::Loaded
            } else {
                plugin_status::State::Stopped
            };
            
            let plugin_info = PluginInfo {
                plugin: Some(ProtoPlugin {
                    plugin_name: record.name.clone(),
                    plugin_id: uuid::Uuid::new_v4().to_string(), // Generate a UUID for now
                    is_active: record.enabled,
                    is_connected: connected_info.is_some(),
                    version: String::new(), // Not stored in PluginRecord
                    capabilities: connected_info.map(|i| i.capabilities.iter().map(|c| format!("{:?}", c)).collect()).unwrap_or_default(),
                    connected_at: connected_info.and_then(|_| Some(prost_types::Timestamp {
                        seconds: Utc::now().timestamp(),
                        nanos: 0,
                    })),
                    metadata: HashMap::new(),
                }),
                status: Some(PluginStatus {
                    state: state as i32,
                    message: if connected_info.is_some() { "Connected".to_string() } else { "Not connected".to_string() },
                    since: None, // TODO: Track state change time
                }),
                granted_capabilities: vec![], // Not stored in PluginRecord
                metrics: Some(PluginMetrics {
                    messages_sent: 0,
                    messages_received: 0,
                    errors_count: 0,
                    cpu_usage_percent: 0.0,
                    memory_bytes: 0,
                    last_activity: None,
                }),
            };
            
            plugin_infos.push(plugin_info);
        }
        
        Ok(Response::new(ListPluginsResponse {
            plugins: plugin_infos,
        }))
    }
    
    async fn get_plugin(
        &self,
        request: Request<GetPluginRequest>,
    ) -> Result<Response<GetPluginResponse>, Status> {
        let req = request.into_inner();
        debug!("Getting plugin: {}", req.plugin_name);
        
        // Find the plugin record
        let plugin_records = self.plugin_manager.get_plugin_records();
        let record = plugin_records.iter()
            .find(|r| r.name == req.plugin_name)
            .ok_or_else(|| Status::not_found(format!("Plugin '{}' not found", req.plugin_name)))?;
        
        // Check if connected
        let connected_plugins = self.plugin_manager.list_connected_plugins().await;
        let connected_info = connected_plugins.iter()
            .find(|p| p.name == record.name);
        
        let state = if let Some(info) = connected_info {
            if info.is_enabled {
                plugin_status::State::Running
            } else {
                plugin_status::State::Loaded
            }
        } else if record.enabled {
            plugin_status::State::Loaded
        } else {
            plugin_status::State::Stopped
        };
        
        let plugin_info = PluginInfo {
            plugin: Some(ProtoPlugin {
                plugin_name: record.name.clone(),
                plugin_id: uuid::Uuid::new_v4().to_string(),
                is_active: record.enabled,
                is_connected: connected_info.is_some(),
                version: String::new(),
                capabilities: connected_info.map(|i| i.capabilities.iter().map(|c| format!("{:?}", c)).collect()).unwrap_or_default(),
                connected_at: connected_info.and_then(|_| Some(prost_types::Timestamp {
                    seconds: Utc::now().timestamp(),
                    nanos: 0,
                })),
                metadata: HashMap::new(),
            }),
            status: Some(PluginStatus {
                state: state as i32,
                message: if connected_info.is_some() { "Connected".to_string() } else { "Not connected".to_string() },
                since: None,
            }),
            granted_capabilities: vec![], // Not stored in PluginRecord
            metrics: Some(PluginMetrics {
                messages_sent: 0,
                messages_received: 0,
                errors_count: 0,
                cpu_usage_percent: 0.0,
                memory_bytes: 0,
                last_activity: None,
            }),
        };
        
        let mut config = HashMap::new();
        if req.include_config {
            // TODO: Load plugin config from storage
        }
        
        Ok(Response::new(GetPluginResponse {
            plugin: Some(plugin_info),
            config,
        }))
    }
    
    async fn enable_plugin(
        &self,
        request: Request<EnablePluginRequest>,
    ) -> Result<Response<EnablePluginResponse>, Status> {
        let req = request.into_inner();
        info!("Enabling plugin: {}", req.plugin_name);
        
        // Enable the plugin
        self.plugin_manager.toggle_plugin(&req.plugin_name, true).await
            .map_err(|e| Status::internal(format!("Failed to enable plugin: {}", e)))?;
        
        // Get updated plugin info
        let plugin_records = self.plugin_manager.get_plugin_records();
        let record = plugin_records.iter()
            .find(|r| r.name == req.plugin_name)
            .ok_or_else(|| Status::not_found(format!("Plugin '{}' not found after enable", req.plugin_name)))?;
        
        let plugin_info = PluginInfo {
            plugin: Some(ProtoPlugin {
                plugin_name: record.name.clone(),
                plugin_id: uuid::Uuid::new_v4().to_string(),
                is_active: record.enabled,
                is_connected: false,
                version: String::new(),
                capabilities: vec![],
                connected_at: None,
                metadata: HashMap::new(),
            }),
            status: Some(PluginStatus {
                state: plugin_status::State::Loaded as i32,
                message: "Enabled".to_string(),
                since: None,
            }),
            granted_capabilities: vec![], // Not stored in PluginRecord
            metrics: Some(PluginMetrics {
                messages_sent: 0,
                messages_received: 0,
                errors_count: 0,
                cpu_usage_percent: 0.0,
                memory_bytes: 0,
                last_activity: None,
            }),
        };
        
        Ok(Response::new(EnablePluginResponse {
            plugin: Some(plugin_info),
        }))
    }
    
    async fn disable_plugin(
        &self,
        request: Request<DisablePluginRequest>,
    ) -> Result<Response<()>, Status> {
        let req = request.into_inner();
        info!("Disabling plugin: {}", req.plugin_name);
        
        // Disable the plugin
        self.plugin_manager.toggle_plugin(&req.plugin_name, false).await
            .map_err(|e| Status::internal(format!("Failed to disable plugin: {}", e)))?;
        
        Ok(Response::new(()))
    }
    
    async fn remove_plugin(
        &self,
        request: Request<RemovePluginRequest>,
    ) -> Result<Response<()>, Status> {
        let req = request.into_inner();
        info!("Removing plugin: {}", req.plugin_name);
        
        // Remove the plugin
        self.plugin_manager.remove_plugin(&req.plugin_name).await
            .map_err(|e| Status::internal(format!("Failed to remove plugin: {}", e)))?;
        
        // TODO: If remove_config or remove_data are true, clean up those as well
        
        Ok(Response::new(()))
    }
    
    async fn reload_plugin(
        &self,
        request: Request<ReloadPluginRequest>,
    ) -> Result<Response<ReloadPluginResponse>, Status> {
        let req = request.into_inner();
        info!("Reloading plugin: {}", req.plugin_name);
        
        // First disable the plugin
        self.plugin_manager.toggle_plugin(&req.plugin_name, false).await
            .map_err(|e| Status::internal(format!("Failed to disable plugin for reload: {}", e)))?;
        
        // Re-enable it
        self.plugin_manager.toggle_plugin(&req.plugin_name, true).await
            .map_err(|e| Status::internal(format!("Failed to re-enable plugin: {}", e)))?;
        
        // Get updated plugin info
        let plugin_records = self.plugin_manager.get_plugin_records();
        let record = plugin_records.iter()
            .find(|r| r.name == req.plugin_name)
            .ok_or_else(|| Status::not_found(format!("Plugin '{}' not found after reload", req.plugin_name)))?;
        
        let plugin_info = PluginInfo {
            plugin: Some(ProtoPlugin {
                plugin_name: record.name.clone(),
                plugin_id: uuid::Uuid::new_v4().to_string(),
                is_active: record.enabled,
                is_connected: false,
                version: String::new(),
                capabilities: vec![],
                connected_at: None,
                metadata: HashMap::new(),
            }),
            status: Some(PluginStatus {
                state: plugin_status::State::Running as i32,
                message: "Reloaded".to_string(),
                since: None,
            }),
            granted_capabilities: vec![], // Not stored in PluginRecord
            metrics: Some(PluginMetrics {
                messages_sent: 0,
                messages_received: 0,
                errors_count: 0,
                cpu_usage_percent: 0.0,
                memory_bytes: 0,
                last_activity: None,
            }),
        };
        
        Ok(Response::new(ReloadPluginResponse {
            plugin: Some(plugin_info),
        }))
    }
    
    async fn install_plugin(
        &self,
        request: Request<InstallPluginRequest>,
    ) -> Result<Response<InstallPluginResponse>, Status> {
        let req = request.into_inner();
        info!("Installing plugin");
        
        // For now, we only support file path installation
        let file_path = match req.source {
            Some(install_plugin_request::Source::FilePath(path)) => path,
            _ => return Err(Status::unimplemented("Only file path installation is currently supported")),
        };
        
        // Load the plugin from file
        self.plugin_manager.load_in_process_plugin(&file_path).await
            .map_err(|e| Status::internal(format!("Failed to install plugin: {}", e)))?;
        
        // Get the plugin name from request
        let plugin_name = if req.plugin_name.is_empty() {
            return Err(Status::invalid_argument("Plugin name must be specified"));
        } else {
            req.plugin_name.clone()
        };
        
        // Auto-enable if requested
        if req.auto_enable {
            self.plugin_manager.toggle_plugin(&plugin_name, true).await
                .map_err(|e| Status::internal(format!("Failed to enable plugin: {}", e)))?;
        }
        
        // Get plugin info
        let plugin_records = self.plugin_manager.get_plugin_records();
        let record = plugin_records.iter()
            .find(|r| r.name == plugin_name)
            .ok_or_else(|| Status::not_found(format!("Plugin '{}' not found after install", plugin_name)))?;
        
        let plugin_info = PluginInfo {
            plugin: Some(ProtoPlugin {
                plugin_name: record.name.clone(),
                plugin_id: uuid::Uuid::new_v4().to_string(),
                is_active: record.enabled,
                is_connected: false,
                version: String::new(),
                capabilities: vec![],
                connected_at: None,
                metadata: HashMap::new(),
            }),
            status: Some(PluginStatus {
                state: if req.auto_enable { 
                    plugin_status::State::Running as i32 
                } else { 
                    plugin_status::State::Loaded as i32 
                },
                message: "Installed".to_string(),
                since: None,
            }),
            granted_capabilities: vec![], // Not stored in PluginRecord
            metrics: Some(PluginMetrics {
                messages_sent: 0,
                messages_received: 0,
                errors_count: 0,
                cpu_usage_percent: 0.0,
                memory_bytes: 0,
                last_activity: None,
            }),
        };
        
        Ok(Response::new(InstallPluginResponse {
            plugin: Some(plugin_info),
            warnings: vec![],
        }))
    }
    
    async fn update_plugin(
        &self,
        request: Request<UpdatePluginRequest>,
    ) -> Result<Response<UpdatePluginResponse>, Status> {
        let req = request.into_inner();
        info!("Updating plugin: {}", req.plugin_name);
        
        // Plugin updates are not yet supported
        Err(Status::unimplemented("Plugin updates are not yet implemented"))
    }
    
    async fn get_plugin_config(
        &self,
        request: Request<GetPluginConfigRequest>,
    ) -> Result<Response<GetPluginConfigResponse>, Status> {
        let req = request.into_inner();
        debug!("Getting config for plugin: {}", req.plugin_name);
        
        // TODO: Implement plugin config storage and retrieval
        // For now, return empty config
        Ok(Response::new(GetPluginConfigResponse {
            config: HashMap::new(),
            definitions: vec![],
        }))
    }
    
    async fn set_plugin_config(
        &self,
        request: Request<SetPluginConfigRequest>,
    ) -> Result<Response<()>, Status> {
        let req = request.into_inner();
        info!("Setting config for plugin: {}", req.plugin_name);
        
        // TODO: Implement plugin config storage
        // For now, just validate the plugin exists
        let plugin_records = self.plugin_manager.get_plugin_records();
        let _record = plugin_records.iter()
            .find(|r| r.name == req.plugin_name)
            .ok_or_else(|| Status::not_found(format!("Plugin '{}' not found", req.plugin_name)))?;
        
        if req.validate_only {
            // Just validate without saving
            return Ok(Response::new(()));
        }
        
        // TODO: Store the config
        
        Ok(Response::new(()))
    }
    
    async fn get_plugin_capabilities(
        &self,
        request: Request<GetPluginCapabilitiesRequest>,
    ) -> Result<Response<GetPluginCapabilitiesResponse>, Status> {
        let req = request.into_inner();
        debug!("Getting capabilities for plugin: {}", req.plugin_name);
        
        // Find the plugin record
        let plugin_records = self.plugin_manager.get_plugin_records();
        let record = plugin_records.iter()
            .find(|r| r.name == req.plugin_name)
            .ok_or_else(|| Status::not_found(format!("Plugin '{}' not found", req.plugin_name)))?;
        
        // TODO: Implement capabilities tracking
        let requested_capabilities = vec![];
        let granted_capabilities = vec![];
        let denied_capabilities = vec![];
        let denial_reasons = HashMap::new();
        
        Ok(Response::new(GetPluginCapabilitiesResponse {
            requested_capabilities,
            granted_capabilities,
            denied_capabilities,
            denial_reasons,
        }))
    }
    
    async fn grant_plugin_capability(
        &self,
        request: Request<GrantPluginCapabilityRequest>,
    ) -> Result<Response<()>, Status> {
        let req = request.into_inner();
        info!("Granting capability '{}' to plugin: {}", req.capability, req.plugin_name);
        
        // Verify plugin exists
        let plugin_records = self.plugin_manager.get_plugin_records();
        let _record = plugin_records.iter()
            .find(|r| r.name == req.plugin_name)
            .ok_or_else(|| Status::not_found(format!("Plugin '{}' not found", req.plugin_name)))?;
        
        // TODO: Implement capability granting
        // Capabilities should be stored separately from PluginRecord
        info!("Would grant capability '{}' to plugin '{}'", req.capability, req.plugin_name);
        
        Ok(Response::new(()))
    }
    
    async fn revoke_plugin_capability(
        &self,
        request: Request<RevokePluginCapabilityRequest>,
    ) -> Result<Response<()>, Status> {
        let req = request.into_inner();
        info!("Revoking capability '{}' from plugin: {}", req.capability, req.plugin_name);
        
        // Verify plugin exists
        let plugin_records = self.plugin_manager.get_plugin_records();
        let _record = plugin_records.iter()
            .find(|r| r.name == req.plugin_name)
            .ok_or_else(|| Status::not_found(format!("Plugin '{}' not found", req.plugin_name)))?;
        
        // TODO: Implement capability revoking
        // Capabilities should be stored separately from PluginRecord
        info!("Would revoke capability '{}' from plugin '{}'", req.capability, req.plugin_name);
        
        Ok(Response::new(()))
    }
    
    async fn send_plugin_message(
        &self,
        request: Request<SendPluginMessageRequest>,
    ) -> Result<Response<SendPluginMessageResponse>, Status> {
        let req = request.into_inner();
        debug!("Sending message to plugin: {}", req.plugin_name);
        
        // TODO: Implement plugin messaging
        // For now, return unimplemented
        Err(Status::unimplemented("Plugin messaging not yet implemented"))
    }
    
    type StreamPluginMessagesStream = tonic::codec::Streaming<PluginMessage>;
    
    async fn stream_plugin_messages(
        &self,
        _request: Request<StreamPluginMessagesRequest>,
    ) -> Result<Response<Self::StreamPluginMessagesStream>, Status> {
        Err(Status::unimplemented("stream_plugin_messages not implemented"))
    }
    
    async fn get_system_status(
        &self,
        _request: Request<GetSystemStatusRequest>,
    ) -> Result<Response<GetSystemStatusResponse>, Status> {
        debug!("Getting system status");
        
        // Get overall status
        let status_data = self.plugin_manager.status().await;
        
        // Get connected plugins count
        let connected_plugins = self.plugin_manager.list_connected_plugins().await;
        let plugin_records = self.plugin_manager.get_plugin_records();
        
        // Count active accounts
        let active_accounts = status_data.account_statuses.iter()
            .filter(|a| a.is_connected)
            .count() as i32;
        
        let system_metrics = SystemMetrics {
            cpu_usage_percent: 0.0, // TODO: Get actual CPU usage
            memory_used_bytes: 0, // TODO: Get actual memory usage
            memory_total_bytes: 0, // TODO: Get total memory
            total_messages_processed: 0, // TODO: Get message count
            messages_per_second: 0.0, // TODO: Calculate message rate
            event_counts: HashMap::new(), // TODO: Track event counts
        };
        
        Ok(Response::new(GetSystemStatusResponse {
            total_plugins: plugin_records.len() as i32,
            active_plugins: connected_plugins.len() as i32,
            uptime_seconds: status_data.uptime_seconds as i64,
            metrics: Some(system_metrics),
            warnings: vec![],
        }))
    }
}