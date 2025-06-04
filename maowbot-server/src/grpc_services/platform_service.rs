use tonic::{Request, Response, Status};
use maowbot_proto::maowbot::services::{
    platform_service_server::PlatformService,
    *,
};
use maowbot_proto::maowbot::common::{Platform, PlatformConfig as ProtoPlatformConfig};
use maowbot_core::platforms::manager::PlatformManager;
use maowbot_common::{
    models::platform::PlatformConfig as PlatformConfigModel,
    traits::repository_traits::PlatformConfigRepository as PlatformConfigRepositoryTrait,
};
use std::sync::Arc;
use std::collections::HashMap;
use uuid::Uuid;
use chrono::Utc;
use tracing::{info, error, debug};
use prost_types;

pub struct PlatformServiceImpl {
    platform_manager: Arc<PlatformManager>,
    platform_config_repo: Arc<dyn PlatformConfigRepositoryTrait + Send + Sync>,
}

impl PlatformServiceImpl {
    pub fn new(
        platform_manager: Arc<PlatformManager>,
        platform_config_repo: Arc<dyn PlatformConfigRepositoryTrait + Send + Sync>,
    ) -> Self {
        Self {
            platform_manager,
            platform_config_repo,
        }
    }
    
    fn platform_config_to_proto(config: &PlatformConfigModel) -> ProtoPlatformConfig {
        ProtoPlatformConfig {
            platform_config_id: config.platform_config_id.to_string(),
            platform: Self::platform_str_to_proto(&config.platform).to_string(),
            client_id: config.client_id.clone().unwrap_or_default(),
            encrypted_client_secret: String::new(), // Don't expose secrets
            scopes: vec![], // We don't store scopes in the model
            additional_config: HashMap::new(), // TODO: Could add extra config
            created_at: Some(prost_types::Timestamp {
                seconds: config.created_at.timestamp(),
                nanos: config.created_at.timestamp_subsec_nanos() as i32,
            }),
            updated_at: Some(prost_types::Timestamp {
                seconds: config.updated_at.timestamp(),
                nanos: config.created_at.timestamp_subsec_nanos() as i32,
            }),
        }
    }
    
    fn platform_str_to_proto(platform_str: &str) -> i32 {
        match platform_str.to_lowercase().as_str() {
            "twitch" | "twitch-irc" => Platform::TwitchIrc as i32,
            "twitch-eventsub" => Platform::TwitchEventsub as i32,
            "discord" => Platform::Discord as i32,
            "vrchat" => Platform::Vrchat as i32,
            "vrchat-pipeline" => Platform::VrchatPipeline as i32,
            "twitch-helix" => Platform::TwitchHelix as i32,
            _ => Platform::Unknown as i32,
        }
    }
    
    fn proto_platform_to_str(platform: i32) -> Result<String, Status> {
        match Platform::try_from(platform) {
            Ok(Platform::TwitchIrc) => Ok("twitch-irc".to_string()),
            Ok(Platform::TwitchEventsub) => Ok("twitch-eventsub".to_string()),
            Ok(Platform::Discord) => Ok("discord".to_string()),
            Ok(Platform::Vrchat) => Ok("vrchat".to_string()),
            Ok(Platform::VrchatPipeline) => Ok("vrchat-pipeline".to_string()),
            Ok(Platform::TwitchHelix) => Ok("twitch-helix".to_string()),
            _ => Err(Status::invalid_argument("Invalid platform")),
        }
    }
}

#[tonic::async_trait]
impl PlatformService for PlatformServiceImpl {
    async fn create_platform_config(
        &self,
        request: Request<CreatePlatformConfigRequest>,
    ) -> Result<Response<CreatePlatformConfigResponse>, Status> {
        let req = request.into_inner();
        let platform_str = Self::proto_platform_to_str(req.platform)?;
        
        info!("Creating platform config for: {}", platform_str);
        
        // Check if config already exists
        if let Ok(existing) = self.platform_config_repo.get_by_platform(&platform_str).await {
            if existing.is_some() {
                return Err(Status::already_exists("Platform config already exists"));
            }
        }
        
        // Create new config
        self.platform_config_repo
            .upsert_platform_config(
                &platform_str,
                Some(req.client_id),
                Some(req.client_secret),
            )
            .await
            .map_err(|e| Status::internal(format!("Failed to create config: {}", e)))?;
        
        // Get the created config
        let config = self.platform_config_repo
            .get_by_platform(&platform_str)
            .await
            .map_err(|e| Status::internal(format!("Failed to get created config: {}", e)))?
            .ok_or_else(|| Status::internal("Config not found after creation"))?;
        
        Ok(Response::new(CreatePlatformConfigResponse {
            config: Some(Self::platform_config_to_proto(&config)),
        }))
    }
    
    async fn get_platform_config(
        &self,
        request: Request<GetPlatformConfigRequest>,
    ) -> Result<Response<GetPlatformConfigResponse>, Status> {
        let req = request.into_inner();
        debug!("Getting platform config by ID: {}", req.platform_config_id);
        
        let config_id = Uuid::parse_str(&req.platform_config_id)
            .map_err(|e| Status::invalid_argument(format!("Invalid config_id: {}", e)))?;
        
        let config = self.platform_config_repo
            .get_platform_config(config_id)
            .await
            .map_err(|e| Status::internal(format!("Failed to get config: {}", e)))?
            .ok_or_else(|| Status::not_found("Platform config not found"))?;
        
        Ok(Response::new(GetPlatformConfigResponse {
            config: Some(Self::platform_config_to_proto(&config)),
        }))
    }
    
    async fn update_platform_config(
        &self,
        request: Request<UpdatePlatformConfigRequest>,
    ) -> Result<Response<UpdatePlatformConfigResponse>, Status> {
        let req = request.into_inner();
        info!("Updating platform config: {}", req.platform_config_id);
        
        let config_id = Uuid::parse_str(&req.platform_config_id)
            .map_err(|e| Status::invalid_argument(format!("Invalid config_id: {}", e)))?;
        
        // Get existing config
        let existing = self.platform_config_repo
            .get_platform_config(config_id)
            .await
            .map_err(|e| Status::internal(format!("Failed to get config: {}", e)))?
            .ok_or_else(|| Status::not_found("Platform config not found"))?;
        
        // Apply updates
        let client_id = if let Some(ref update_mask) = req.update_mask {
            if update_mask.paths.contains(&"client_id".to_string()) {
                req.config.as_ref().map(|c| c.client_id.clone())
            } else {
                existing.client_id
            }
        } else {
            req.config.as_ref().map(|c| c.client_id.clone())
        };
        
        let client_secret = existing.client_secret.clone();
        
        // Update the config
        self.platform_config_repo
            .upsert_platform_config(
                &existing.platform,
                client_id,
                client_secret,
            )
            .await
            .map_err(|e| Status::internal(format!("Failed to update config: {}", e)))?;
        
        // Get updated config
        let updated = self.platform_config_repo
            .get_platform_config(config_id)
            .await
            .map_err(|e| Status::internal(format!("Failed to get updated config: {}", e)))?
            .ok_or_else(|| Status::internal("Config not found after update"))?;
        
        Ok(Response::new(UpdatePlatformConfigResponse {
            config: Some(Self::platform_config_to_proto(&updated)),
        }))
    }
    
    async fn delete_platform_config(
        &self,
        request: Request<DeletePlatformConfigRequest>,
    ) -> Result<Response<()>, Status> {
        let req = request.into_inner();
        info!("Deleting platform config: {}", req.platform_config_id);
        
        let config_id = Uuid::parse_str(&req.platform_config_id)
            .map_err(|e| Status::invalid_argument(format!("Invalid config_id: {}", e)))?;
        
        self.platform_config_repo
            .delete_platform_config(config_id)
            .await
            .map_err(|e| Status::internal(format!("Failed to delete config: {}", e)))?;
        
        Ok(Response::new(()))
    }
    
    async fn list_platform_configs(
        &self,
        request: Request<ListPlatformConfigsRequest>,
    ) -> Result<Response<ListPlatformConfigsResponse>, Status> {
        let req = request.into_inner();
        debug!("Listing platform configs");
        
        let platform_filter = if req.platforms.is_empty() {
            None
        } else {
            // Just use the first platform for filtering since our repo method takes one
            req.platforms.first()
                .and_then(|&p| Self::proto_platform_to_str(p).ok())
        };
        let platform_filter = platform_filter.as_deref();
        
        let configs = self.platform_config_repo
            .list_platform_configs(platform_filter)
            .await
            .map_err(|e| Status::internal(format!("Failed to list configs: {}", e)))?;
        
        let proto_configs: Vec<ProtoPlatformConfig> = configs.into_iter()
            .map(|c| Self::platform_config_to_proto(&c))
            .collect();
        
        Ok(Response::new(ListPlatformConfigsResponse {
            configs: proto_configs,
            page: None, // TODO: Implement pagination
        }))
    }
    
    async fn start_platform_runtime(
        &self,
        request: Request<StartPlatformRuntimeRequest>,
    ) -> Result<Response<StartPlatformRuntimeResponse>, Status> {
        let req = request.into_inner();
        info!("Starting platform runtime: {} for account: {}", req.platform, req.account_name);
        
        self.platform_manager
            .start_platform_runtime(&req.platform, &req.account_name)
            .await
            .map_err(|e| Status::internal(format!("Failed to start runtime: {}", e)))?;
        
        Ok(Response::new(StartPlatformRuntimeResponse {
            runtime_id: format!("{}-{}", req.platform, req.account_name),
            status: Some(RuntimeStatus {
                state: runtime_status::State::Running as i32,
                message: "Started".to_string(),
                since: None,
            }),
            error_message: String::new(),
        }))
    }
    
    async fn stop_platform_runtime(
        &self,
        request: Request<StopPlatformRuntimeRequest>,
    ) -> Result<Response<()>, Status> {
        let req = request.into_inner();
        info!("Stopping platform runtime: {} for account: {}", req.platform, req.account_name);
        
        self.platform_manager
            .stop_platform_runtime(&req.platform, &req.account_name)
            .await
            .map_err(|e| Status::internal(format!("Failed to stop runtime: {}", e)))?;
        
        Ok(Response::new(()))
    }
    
    async fn restart_platform_runtime(
        &self,
        request: Request<RestartPlatformRuntimeRequest>,
    ) -> Result<Response<RestartPlatformRuntimeResponse>, Status> {
        let req = request.into_inner();
        info!("Restarting platform runtime: {} for account: {}", req.platform, req.account_name);
        
        // Stop the runtime
        self.platform_manager
            .stop_platform_runtime(&req.platform, &req.account_name)
            .await
            .map_err(|e| Status::internal(format!("Failed to stop runtime: {}", e)))?;
        
        // Wait a moment for cleanup
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        
        // Start it again
        self.platform_manager
            .start_platform_runtime(&req.platform, &req.account_name)
            .await
            .map_err(|e| Status::internal(format!("Failed to start runtime: {}", e)))?;
        
        Ok(Response::new(RestartPlatformRuntimeResponse {
            runtime_id: format!("{}-{}", req.platform, req.account_name),
            status: Some(RuntimeStatus {
                state: runtime_status::State::Running as i32,
                message: "Restarted".to_string(),
                since: None,
            }),
        }))
    }
    
    async fn get_platform_runtime_status(
        &self,
        request: Request<GetPlatformRuntimeStatusRequest>,
    ) -> Result<Response<GetPlatformRuntimeStatusResponse>, Status> {
        let req = request.into_inner();
        debug!("Getting runtime status for: {} {}", req.platform, req.account_name);
        
        let platform = &req.platform;
        let account_name = &req.account_name;
        
        // Check if runtime is active
        let pm = &self.platform_manager;
        let runtimes_guard = pm.active_runtimes.lock().await;
        let is_active = runtimes_guard.contains_key(&(platform.to_string(), account_name.to_string()));
        drop(runtimes_guard);
        
        let status = if is_active {
            RuntimeStatus {
                state: runtime_status::State::Running as i32,
                message: "Running".to_string(),
                since: None, // TODO: Track start time
            }
        } else {
            RuntimeStatus {
                state: runtime_status::State::Stopped as i32,
                message: "Not running".to_string(),
                since: None,
            }
        };
        
        Ok(Response::new(GetPlatformRuntimeStatusResponse {
            info: Some(RuntimeInfo {
                runtime_id: format!("{}-{}", platform, account_name),
                platform: platform.to_string(),
                account_name: account_name.to_string(),
                    started_at: None, // TODO: Track start time
                uptime_seconds: 0, // TODO: Calculate uptime
                stats: Some(RuntimeStatistics {
                    messages_sent: 0,
                    messages_received: 0,
                    events_processed: 0,
                    errors_count: 0,
                    last_activity: None,
                }),
                platform_specific: HashMap::new(),
            }),
            status: Some(status),
        }))
    }
    
    async fn list_active_runtimes(
        &self,
        request: Request<ListActiveRuntimesRequest>,
    ) -> Result<Response<ListActiveRuntimesResponse>, Status> {
        let req = request.into_inner();
        debug!("Listing active runtimes");
        
        let pm = &self.platform_manager;
        let runtimes_guard = pm.active_runtimes.lock().await;
        let mut runtime_infos = Vec::new();
        
        for ((platform, account_name), handle) in runtimes_guard.iter() {
            // Filter by platform if specified
            if !req.platforms.is_empty() {
                let platform_proto = Self::platform_str_to_proto(platform);
                if !req.platforms.contains(&platform_proto) {
                    continue;
                }
            }
            
            // Calculate uptime
            let uptime_seconds = (chrono::Utc::now() - handle.started_at).num_seconds();
            
            runtime_infos.push(RuntimeInfo {
                runtime_id: format!("{}-{}", platform, account_name),
                platform: platform.clone(),
                account_name: account_name.clone(),
                started_at: Some(prost_types::Timestamp {
                    seconds: handle.started_at.timestamp(),
                    nanos: handle.started_at.timestamp_subsec_nanos() as i32,
                }),
                uptime_seconds,
                stats: Some(RuntimeStatistics {
                    messages_sent: 0,
                    messages_received: 0,
                    events_processed: 0,
                    errors_count: 0,
                    last_activity: None,
                }),
                platform_specific: HashMap::new(),
            });
        }
        
        drop(runtimes_guard);
        
        let mut runtime_counts = HashMap::new();
        for info in &runtime_infos {
            *runtime_counts.entry(info.platform.clone()).or_insert(0) += 1;
        }
        
        Ok(Response::new(ListActiveRuntimesResponse {
            runtimes: runtime_infos,
            runtime_counts,
        }))
    }
    
    async fn get_platform_capabilities(
        &self,
        request: Request<GetPlatformCapabilitiesRequest>,
    ) -> Result<Response<GetPlatformCapabilitiesResponse>, Status> {
        let req = request.into_inner();
        let platform = Platform::try_from(req.platform)
            .map_err(|_| Status::invalid_argument("Invalid platform"))?;
        
        debug!("Getting capabilities for platform: {:?}", platform);
        
        let (capabilities, required_scopes, limitations) = match platform {
            Platform::TwitchIrc => {
                let caps = vec![
                    Capability {
                        name: "chat".to_string(),
                        description: "Send and receive chat messages".to_string(),
                        requires_auth: true,
                        required_roles: vec![],
                    },
                    Capability {
                        name: "moderation".to_string(),
                        description: "Moderate chat (timeout, ban)".to_string(),
                        requires_auth: true,
                        required_roles: vec!["moderator".to_string()],
                    },
                ];
                let scopes = vec!["chat:read".to_string(), "chat:edit".to_string()];
                let limits = HashMap::from([
                    ("rate_limit".to_string(), "20 messages per 30 seconds".to_string()),
                    ("note".to_string(), "Legacy chat system, consider using EventSub".to_string()),
                ]);
                (caps, scopes, limits)
            }
            Platform::TwitchEventsub => {
                let caps = vec![
                    Capability {
                        name: "chat".to_string(),
                        description: "Chat events via EventSub".to_string(),
                        requires_auth: true,
                        required_roles: vec![],
                    },
                    Capability {
                        name: "channel_points".to_string(),
                        description: "Channel point redemptions".to_string(),
                        requires_auth: true,
                        required_roles: vec!["broadcaster".to_string()],
                    },
                    Capability {
                        name: "subscriptions".to_string(),
                        description: "Subscription events".to_string(),
                        requires_auth: true,
                        required_roles: vec![],
                    },
                ];
                let scopes = vec![
                    "channel:read:redemptions".to_string(),
                    "channel:read:subscriptions".to_string(),
                ];
                let limits = HashMap::from([
                    ("note".to_string(), "Modern event-driven system".to_string()),
                ]);
                (caps, scopes, limits)
            }
            Platform::Discord => {
                let caps = vec![
                    Capability {
                        name: "chat".to_string(),
                        description: "Send and receive messages".to_string(),
                        requires_auth: true,
                        required_roles: vec![],
                    },
                    Capability {
                        name: "voice".to_string(),
                        description: "Voice channel support".to_string(),
                        requires_auth: true,
                        required_roles: vec![],
                    },
                ];
                let scopes = vec!["bot".to_string()];
                let limits = HashMap::from([
                    ("rate_limit".to_string(), "5 messages per 5 seconds per channel".to_string()),
                    ("note".to_string(), "Supports slash commands".to_string()),
                ]);
                (caps, scopes, limits)
            }
            Platform::Vrchat => {
                let caps = vec![
                    Capability {
                        name: "presence".to_string(),
                        description: "Track presence and world info".to_string(),
                        requires_auth: true,
                        required_roles: vec![],
                    },
                    Capability {
                        name: "osc".to_string(),
                        description: "OSC integration for avatar control".to_string(),
                        requires_auth: false,
                        required_roles: vec![],
                    },
                ];
                let scopes = vec![];
                let limits = HashMap::from([
                    ("rate_limit".to_string(), "API rate limits apply".to_string()),
                    ("note".to_string(), "Supports OSC integration".to_string()),
                ]);
                (caps, scopes, limits)
            }
            _ => {
                let caps = vec![];
                let scopes = vec![];
                let limits = HashMap::from([
                    ("note".to_string(), "Unknown platform".to_string()),
                ]);
                (caps, scopes, limits)
            }
        };
        
        Ok(Response::new(GetPlatformCapabilitiesResponse {
            capabilities,
            required_scopes,
            limitations,
        }))
    }
    
    type StreamPlatformEventsStream = tonic::codec::Streaming<PlatformEvent>;
    
    async fn stream_platform_events(
        &self,
        _request: Request<StreamPlatformEventsRequest>,
    ) -> Result<Response<Self::StreamPlatformEventsStream>, Status> {
        Err(Status::unimplemented("stream_platform_events not implemented"))
    }
}