use tonic::{Request, Response, Status};
use maowbot_proto::maowbot::services::{redeem_service_server::RedeemService, *};
use maowbot_proto::maowbot::common;
use maowbot_common::traits::repository_traits::{RedeemRepository, RedeemUsageRepository};
use maowbot_core::services::twitch::redeem_service::RedeemService as CoreRedeemService;
use std::sync::Arc;
use std::collections::HashMap;
use uuid::Uuid;
use chrono::Utc;
use tracing::{info, error, debug};
use prost_types;

pub struct RedeemServiceImpl {
    redeem_repo: Arc<dyn RedeemRepository + Send + Sync>,
    redeem_usage_repo: Arc<dyn RedeemUsageRepository + Send + Sync>,
    redeem_service: Arc<CoreRedeemService>,
}

impl RedeemServiceImpl {
    pub fn new(
        redeem_repo: Arc<dyn RedeemRepository + Send + Sync>,
        redeem_usage_repo: Arc<dyn RedeemUsageRepository + Send + Sync>,
        redeem_service: Arc<CoreRedeemService>,
    ) -> Self {
        Self {
            redeem_repo,
            redeem_usage_repo,
            redeem_service,
        }
    }
    
    fn redeem_to_proto(rd: &maowbot_common::models::redeem::Redeem) -> common::Redeem {
        let mut metadata = HashMap::new();
        metadata.insert("dynamic_pricing".to_string(), rd.dynamic_pricing.to_string());
        metadata.insert("active_offline".to_string(), rd.active_offline.to_string());
        metadata.insert("is_managed".to_string(), rd.is_managed.to_string());
        metadata.insert("is_input_required".to_string(), rd.is_input_required.to_string());
        
        if let Some(plugin_name) = &rd.plugin_name {
            metadata.insert("plugin_name".to_string(), plugin_name.clone());
        }
        if let Some(command_name) = &rd.command_name {
            metadata.insert("command_name".to_string(), command_name.clone());
        }
        if let Some(cred_id) = &rd.active_credential_id {
            metadata.insert("active_credential_id".to_string(), cred_id.to_string());
        }
        if let Some(prompt_text) = &rd.redeem_prompt_text {
            metadata.insert("prompt_text".to_string(), prompt_text.clone());
        }
        
        common::Redeem {
            redeem_id: rd.redeem_id.to_string(),
            platform: rd.platform.clone(),
            reward_id: rd.reward_id.clone(),
            reward_name: rd.reward_name.clone(),
            cost: rd.cost,
            is_active: rd.is_active,
            is_dynamic: rd.dynamic_pricing,
            handler: rd.plugin_name.clone().unwrap_or_else(|| 
                rd.command_name.clone().unwrap_or_default()
            ),
            created_at: Some(prost_types::Timestamp {
                seconds: rd.created_at.timestamp(),
                nanos: rd.created_at.timestamp_subsec_nanos() as i32,
            }),
            updated_at: Some(prost_types::Timestamp {
                seconds: rd.updated_at.timestamp(),
                nanos: rd.updated_at.timestamp_subsec_nanos() as i32,
            }),
            metadata,
        }
    }
    
    fn proto_to_redeem(proto: &common::Redeem) -> Result<maowbot_common::models::redeem::Redeem, Status> {
        let redeem_id = if proto.redeem_id.is_empty() {
            Uuid::new_v4()
        } else {
            Uuid::parse_str(&proto.redeem_id)
                .map_err(|e| Status::invalid_argument(format!("Invalid redeem ID: {}", e)))?
        };
        
        // Parse handler field to determine plugin_name or command_name
        let (plugin_name, command_name) = if proto.handler.is_empty() {
            (None, None)
        } else if proto.handler.starts_with("plugin:") {
            (Some(proto.handler.trim_start_matches("plugin:").to_string()), None)
        } else {
            (None, Some(proto.handler.clone()))
        };
        
        // Extract additional fields from metadata
        let dynamic_pricing = proto.metadata.get("dynamic_pricing")
            .and_then(|s| s.parse::<bool>().ok())
            .unwrap_or(proto.is_dynamic);
            
        let active_offline = proto.metadata.get("active_offline")
            .and_then(|s| s.parse::<bool>().ok())
            .unwrap_or(false);
            
        let is_managed = proto.metadata.get("is_managed")
            .and_then(|s| s.parse::<bool>().ok())
            .unwrap_or(false);
            
        let is_input_required = proto.metadata.get("is_input_required")
            .and_then(|s| s.parse::<bool>().ok())
            .unwrap_or(false);
            
        let active_credential_id = proto.metadata.get("active_credential_id")
            .and_then(|id| Uuid::parse_str(id).ok());
            
        let redeem_prompt_text = proto.metadata.get("prompt_text")
            .filter(|s| !s.is_empty())
            .cloned();
        
        Ok(maowbot_common::models::redeem::Redeem {
            redeem_id,
            platform: proto.platform.clone(),
            reward_id: proto.reward_id.clone(),
            reward_name: proto.reward_name.clone(),
            cost: proto.cost,
            is_active: proto.is_active,
            dynamic_pricing,
            active_offline,
            is_managed,
            plugin_name: plugin_name.or_else(|| proto.metadata.get("plugin_name").cloned()),
            command_name: command_name.or_else(|| proto.metadata.get("command_name").cloned()),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            active_credential_id,
            is_input_required,
            redeem_prompt_text,
        })
    }
}

#[tonic::async_trait]
impl RedeemService for RedeemServiceImpl {
    async fn list_redeems(&self, request: Request<ListRedeemsRequest>) -> Result<Response<ListRedeemsResponse>, Status> {
        let req = request.into_inner();
        debug!("Listing redeems - platform: {:?}, active_only: {}, dynamic_only: {}", 
               req.platform, req.active_only, req.dynamic_only);
        
        // Get redeems for platform
        let redeems = if req.platform.is_empty() {
            // If no platform specified, we'd need to list all - for now return empty
            vec![]
        } else {
            self.redeem_repo.list_redeems(&req.platform).await
                .map_err(|e| Status::internal(format!("Failed to list redeems: {}", e)))?
        };
        
        // Filter by active_only and dynamic_only if requested
        let filtered_redeems: Vec<_> = redeems.into_iter()
            .filter(|rd| !req.active_only || rd.is_active)
            .filter(|rd| !req.dynamic_only || rd.dynamic_pricing)
            .collect();
        
        // Convert to proto format
        let mut redeem_infos = Vec::new();
        for rd in filtered_redeems {
            // TODO: Get real statistics from usage data
            let stats = RedeemStatistics {
                total_redemptions: 0,
                unique_users: 0,
                last_redeemed: None,
                average_time_between_redemptions: 0.0,
                total_points_spent: 0,
            };
            
            // TODO: Get sync status
            let sync_status = SyncStatus {
                is_synced: true,
                last_sync: Some(prost_types::Timestamp {
                    seconds: Utc::now().timestamp(),
                    nanos: 0,
                }),
                sync_error: String::new(),
                platform_exists: true,
            };
            
            // TODO: Get linked OSC triggers
            let linked_triggers = vec![];
            
            redeem_infos.push(RedeemInfo {
                redeem: Some(Self::redeem_to_proto(&rd)),
                stats: Some(stats),
                sync_status: Some(sync_status),
                linked_triggers,
            });
        }
        
        // TODO: Implement proper pagination
        Ok(Response::new(ListRedeemsResponse {
            redeems: redeem_infos,
            page: None,
        }))
    }
    async fn get_redeem(&self, request: Request<GetRedeemRequest>) -> Result<Response<GetRedeemResponse>, Status> {
        let req = request.into_inner();
        let redeem_id = Uuid::parse_str(&req.redeem_id)
            .map_err(|e| Status::invalid_argument(format!("Invalid redeem ID: {}", e)))?;
        
        debug!("Getting redeem: {}", redeem_id);
        
        let rd = self.redeem_repo.get_redeem_by_id(redeem_id).await
            .map_err(|e| Status::internal(format!("Failed to get redeem: {}", e)))?;
        
        let rd = match rd {
            Some(r) => r,
            None => return Err(Status::not_found("Redeem not found")),
        };
        
        // Get usage data if requested
        let mut recent_usage = Vec::new();
        if req.include_usage {
            let usage_data = self.redeem_usage_repo.list_usage_for_redeem(redeem_id, 10).await
                .map_err(|e| Status::internal(format!("Failed to get usage data: {}", e)))?;
            
            for usage in usage_data {
                // Parse usage data JSON to get status and response
                let (status, handler_response, user_input) = if let Some(data) = &usage.usage_data {
                    let status_str = data.get("status").and_then(|v| v.as_str()).unwrap_or("unknown");
                    let status = match status_str {
                        "pending" => RedeemRedemptionStatus::Pending,
                        "fulfilled" => RedeemRedemptionStatus::Fulfilled,
                        "canceled" => RedeemRedemptionStatus::Canceled,
                        "failed" => RedeemRedemptionStatus::Failed,
                        _ => RedeemRedemptionStatus::Unknown,
                    };
                    let response = data.get("response").and_then(|v| v.as_str()).unwrap_or("").to_string();
                    let input = data.get("user_input").and_then(|v| v.as_str()).unwrap_or("").to_string();
                    (status, response, input)
                } else {
                    (RedeemRedemptionStatus::Unknown, String::new(), String::new())
                };
                
                recent_usage.push(RedeemUsageEntry {
                    user_id: usage.user_id.to_string(),
                    platform_user_id: String::new(), // TODO: Look up platform user ID
                    redeemed_at: Some(prost_types::Timestamp {
                        seconds: usage.used_at.timestamp(),
                        nanos: usage.used_at.timestamp_subsec_nanos() as i32,
                    }),
                    user_input,
                    status: status as i32,
                    handler_response,
                });
            }
        }
        
        // Build response
        let stats = RedeemStatistics {
            total_redemptions: recent_usage.len() as i64,
            unique_users: 0, // TODO: Calculate unique users
            last_redeemed: recent_usage.first().and_then(|u| u.redeemed_at.clone()),
            average_time_between_redemptions: 0.0,
            total_points_spent: rd.cost as i64 * recent_usage.len() as i64,
        };
        
        let sync_status = SyncStatus {
            is_synced: true,
            last_sync: Some(prost_types::Timestamp {
                seconds: Utc::now().timestamp(),
                nanos: 0,
            }),
            sync_error: String::new(),
            platform_exists: true,
        };
        
        let redeem_info = RedeemInfo {
            redeem: Some(Self::redeem_to_proto(&rd)),
            stats: Some(stats),
            sync_status: Some(sync_status),
            linked_triggers: vec![],
        };
        
        Ok(Response::new(GetRedeemResponse {
            redeem: Some(redeem_info),
            recent_usage,
        }))
    }
    async fn create_redeem(&self, request: Request<CreateRedeemRequest>) -> Result<Response<CreateRedeemResponse>, Status> {
        let req = request.into_inner();
        let proto_rd = req.redeem.ok_or_else(|| Status::invalid_argument("Redeem is required"))?;
        
        info!("Creating redeem: {} on platform {}", proto_rd.reward_name, proto_rd.platform);
        
        // Validate redeem
        if proto_rd.reward_name.is_empty() {
            return Err(Status::invalid_argument("Reward name cannot be empty"));
        }
        if proto_rd.reward_id.is_empty() {
            return Err(Status::invalid_argument("Reward ID cannot be empty"));
        }
        if proto_rd.cost < 0 {
            return Err(Status::invalid_argument("Cost cannot be negative"));
        }
        
        // Check if redeem already exists
        let existing = self.redeem_repo.get_redeem_by_reward_id(&proto_rd.platform, &proto_rd.reward_id).await
            .map_err(|e| Status::internal(format!("Failed to check existing redeem: {}", e)))?;
        
        if existing.is_some() {
            return Err(Status::already_exists(format!("Redeem with reward_id '{}' already exists on platform {}", 
                proto_rd.reward_id, proto_rd.platform)));
        }
        
        // Convert proto to model
        let mut rd = Self::proto_to_redeem(&proto_rd)?;
        rd.created_at = Utc::now();
        rd.updated_at = Utc::now();
        
        // Create the redeem
        self.redeem_repo.create_redeem(&rd).await
            .map_err(|e| Status::internal(format!("Failed to create redeem: {}", e)))?;
        
        // TODO: Sync to platform if requested
        let synced = req.sync_to_platform && false; // Not implemented yet
        
        Ok(Response::new(CreateRedeemResponse {
            redeem: Some(Self::redeem_to_proto(&rd)),
            synced,
        }))
    }
    async fn update_redeem(&self, request: Request<UpdateRedeemRequest>) -> Result<Response<UpdateRedeemResponse>, Status> {
        let req = request.into_inner();
        let redeem_id = Uuid::parse_str(&req.redeem_id)
            .map_err(|e| Status::invalid_argument(format!("Invalid redeem ID: {}", e)))?;
        
        info!("Updating redeem: {}", redeem_id);
        
        // Get existing redeem
        let existing = self.redeem_repo.get_redeem_by_id(redeem_id).await
            .map_err(|e| Status::internal(format!("Failed to get redeem: {}", e)))?;
        
        let mut existing = match existing {
            Some(r) => r,
            None => return Err(Status::not_found("Redeem not found")),
        };
        
        let proto_rd = req.redeem.ok_or_else(|| Status::invalid_argument("Redeem is required"))?;
        
        // Apply updates based on field mask
        if let Some(mask) = req.update_mask {
            for path in &mask.paths {
                match path.as_str() {
                    "reward_name" => existing.reward_name = proto_rd.reward_name.clone(),
                    "cost" => existing.cost = proto_rd.cost,
                    "is_active" => existing.is_active = proto_rd.is_active,
                    "dynamic_pricing" => existing.dynamic_pricing = proto_rd.metadata.get("dynamic_pricing")
                        .and_then(|s| s.parse::<bool>().ok())
                        .unwrap_or(existing.dynamic_pricing),
                    "active_offline" => existing.active_offline = proto_rd.metadata.get("active_offline")
                        .and_then(|s| s.parse::<bool>().ok())
                        .unwrap_or(existing.active_offline),
                    "is_input_required" => existing.is_input_required = proto_rd.metadata.get("is_input_required")
                        .and_then(|s| s.parse::<bool>().ok())
                        .unwrap_or(existing.is_input_required),
                    "prompt_text" => existing.redeem_prompt_text = proto_rd.metadata.get("prompt_text")
                        .filter(|s| !s.is_empty())
                        .cloned(),
                    "active_credential_id" => {
                        existing.active_credential_id = if let Some(id) = proto_rd.metadata.get("active_credential_id") {
                            Some(Uuid::parse_str(id)
                                .map_err(|e| Status::invalid_argument(format!("Invalid credential ID: {}", e)))?)
                        } else {
                            None
                        };
                    }
                    _ => debug!("Unknown field in update mask: {}", path),
                }
            }
        } else {
            // No field mask - update all fields except IDs and timestamps
            existing.reward_name = proto_rd.reward_name.clone();
            existing.cost = proto_rd.cost;
            existing.is_active = proto_rd.is_active;
            existing.dynamic_pricing = proto_rd.metadata.get("dynamic_pricing")
                .and_then(|s| s.parse::<bool>().ok())
                .unwrap_or(existing.dynamic_pricing);
            existing.active_offline = proto_rd.metadata.get("active_offline")
                .and_then(|s| s.parse::<bool>().ok())
                .unwrap_or(existing.active_offline);
            existing.is_input_required = proto_rd.metadata.get("is_input_required")
                .and_then(|s| s.parse::<bool>().ok())
                .unwrap_or(existing.is_input_required);
            existing.redeem_prompt_text = proto_rd.metadata.get("prompt_text")
                .filter(|s| !s.is_empty())
                .cloned();
            existing.active_credential_id = proto_rd.metadata.get("active_credential_id")
                .and_then(|id| Uuid::parse_str(id).ok());
        }
        
        existing.updated_at = Utc::now();
        
        // Update the redeem
        self.redeem_repo.update_redeem(&existing).await
            .map_err(|e| Status::internal(format!("Failed to update redeem: {}", e)))?;
        
        // TODO: Sync to platform if requested
        let synced = req.sync_to_platform && false; // Not implemented yet
        
        Ok(Response::new(UpdateRedeemResponse {
            redeem: Some(Self::redeem_to_proto(&existing)),
            synced,
        }))
    }
    async fn delete_redeem(&self, request: Request<DeleteRedeemRequest>) -> Result<Response<()>, Status> {
        let req = request.into_inner();
        let redeem_id = Uuid::parse_str(&req.redeem_id)
            .map_err(|e| Status::invalid_argument(format!("Invalid redeem ID: {}", e)))?;
        
        info!("Deleting redeem: {}", redeem_id);
        
        // TODO: Remove from platform if requested
        if req.remove_from_platform {
            debug!("Platform removal not yet implemented");
        }
        
        // Delete the redeem
        self.redeem_repo.delete_redeem(redeem_id).await
            .map_err(|e| Status::internal(format!("Failed to delete redeem: {}", e)))?;
        
        Ok(Response::new(()))
    }
    async fn batch_list_redeems(&self, request: Request<BatchListRedeemsRequest>) -> Result<Response<BatchListRedeemsResponse>, Status> {
        let req = request.into_inner();
        debug!("Batch listing redeems for {} platforms", req.platforms.len());
        
        let mut by_platform = HashMap::new();
        let mut all_redeems = Vec::new();
        
        // Get redeems for each platform
        for platform in &req.platforms {
            let redeems = self.redeem_repo.list_redeems(platform).await
                .map_err(|e| Status::internal(format!("Failed to list redeems for {}: {}", platform, e)))?;
            
            let active_count = redeems.iter().filter(|r| r.is_active).count() as i32;
            let total_count = redeems.len() as i32;
            let synced_count = total_count; // TODO: Track actual sync status
            
            let mut redeem_infos = Vec::new();
            for rd in redeems {
                let stats = RedeemStatistics {
                    total_redemptions: 0,
                    unique_users: 0,
                    last_redeemed: None,
                    average_time_between_redemptions: 0.0,
                    total_points_spent: 0,
                };
                
                let sync_status = if req.include_sync_status {
                    Some(SyncStatus {
                        is_synced: true,
                        last_sync: Some(prost_types::Timestamp {
                            seconds: Utc::now().timestamp(),
                            nanos: 0,
                        }),
                        sync_error: String::new(),
                        platform_exists: true,
                    })
                } else {
                    None
                };
                
                let info = RedeemInfo {
                    redeem: Some(Self::redeem_to_proto(&rd)),
                    stats: Some(stats),
                    sync_status,
                    linked_triggers: vec![],
                };
                
                redeem_infos.push(info.clone());
                if !req.group_by_platform {
                    all_redeems.push(info);
                }
            }
            
            if req.group_by_platform {
                by_platform.insert(platform.clone(), RedeemList {
                    redeems: redeem_infos,
                    active_count,
                    total_count,
                    synced_count,
                });
            }
        }
        
        Ok(Response::new(BatchListRedeemsResponse {
            by_platform,
            all_redeems,
        }))
    }
    async fn batch_update_redeems(&self, request: Request<BatchUpdateRedeemsRequest>) -> Result<Response<BatchUpdateRedeemsResponse>, Status> {
        let req = request.into_inner();
        info!("Batch updating {} redeems", req.updates.len());
        
        let mut results = Vec::new();
        let mut success_count = 0;
        let mut failure_count = 0;
        
        for update in req.updates {
            let redeem_id = match Uuid::parse_str(&update.redeem_id) {
                Ok(id) => id,
                Err(e) => {
                    failure_count += 1;
                    results.push(UpdateRedeemResult {
                        redeem_id: update.redeem_id,
                        success: false,
                        redeem: None,
                        synced: false,
                        error_message: format!("Invalid redeem ID: {}", e),
                    });
                    if req.atomic {
                        return Err(Status::invalid_argument("Atomic operation failed due to invalid redeem ID"));
                    }
                    continue;
                }
            };
            
            // Get existing redeem
            let existing = match self.redeem_repo.get_redeem_by_id(redeem_id).await {
                Ok(Some(r)) => r,
                Ok(None) => {
                    failure_count += 1;
                    results.push(UpdateRedeemResult {
                        redeem_id: update.redeem_id,
                        success: false,
                        redeem: None,
                        synced: false,
                        error_message: "Redeem not found".to_string(),
                    });
                    if req.atomic {
                        return Err(Status::not_found("Atomic operation failed: redeem not found"));
                    }
                    continue;
                }
                Err(e) => {
                    failure_count += 1;
                    results.push(UpdateRedeemResult {
                        redeem_id: update.redeem_id,
                        success: false,
                        redeem: None,
                        synced: false,
                        error_message: format!("Failed to get redeem: {}", e),
                    });
                    if req.atomic {
                        return Err(Status::internal("Atomic operation failed"));
                    }
                    continue;
                }
            };
            
            // Apply the update
            let mut updated = existing;
            if let Some(proto_rd) = update.redeem {
                if let Some(mask) = update.update_mask {
                    // Apply field mask updates
                    for path in &mask.paths {
                        match path.as_str() {
                            "reward_name" => updated.reward_name = proto_rd.reward_name.clone(),
                            "cost" => updated.cost = proto_rd.cost,
                            "is_active" => updated.is_active = proto_rd.is_active,
                            "dynamic_pricing" => updated.dynamic_pricing = proto_rd.metadata.get("dynamic_pricing")
                                .and_then(|s| s.parse::<bool>().ok())
                                .unwrap_or(updated.dynamic_pricing),
                            "active_offline" => updated.active_offline = proto_rd.metadata.get("active_offline")
                                .and_then(|s| s.parse::<bool>().ok())
                                .unwrap_or(updated.active_offline),
                            _ => {}
                        }
                    }
                }
            }
            
            updated.updated_at = Utc::now();
            
            // Save the update
            match self.redeem_repo.update_redeem(&updated).await {
                Ok(_) => {
                    success_count += 1;
                    let synced = req.sync_all && false; // TODO: Implement sync
                    results.push(UpdateRedeemResult {
                        redeem_id: update.redeem_id,
                        success: true,
                        redeem: Some(Self::redeem_to_proto(&updated)),
                        synced,
                        error_message: String::new(),
                    });
                }
                Err(e) => {
                    failure_count += 1;
                    results.push(UpdateRedeemResult {
                        redeem_id: update.redeem_id,
                        success: false,
                        redeem: None,
                        synced: false,
                        error_message: format!("Failed to update: {}", e),
                    });
                    if req.atomic {
                        return Err(Status::internal("Atomic operation failed during update"));
                    }
                }
            }
        }
        
        Ok(Response::new(BatchUpdateRedeemsResponse {
            results,
            success_count,
            failure_count,
        }))
    }
    async fn sync_redeems(&self, request: Request<SyncRedeemsRequest>) -> Result<Response<SyncRedeemsResponse>, Status> {
        let req = request.into_inner();
        info!("Syncing redeems - platforms: {:?}, direction: {:?}, dry_run: {}", 
              req.platforms, req.direction, req.dry_run);
        
        // TODO: Implement actual sync logic with platforms
        // For now, return a mock response
        
        let mut results = Vec::new();
        let created_count = 0;
        let updated_count = 0;
        let deleted_count = 0;
        let error_count = 0;
        
        if req.dry_run {
            debug!("Dry run mode - no actual changes will be made");
        }
        
        // Mock some sync results
        for platform in &req.platforms {
            let redeems = self.redeem_repo.list_redeems(platform).await
                .map_err(|e| Status::internal(format!("Failed to list redeems: {}", e)))?;
            
            for rd in redeems.iter().take(3) { // Just mock first 3
                results.push(SyncResult {
                    redeem_id: rd.redeem_id.to_string(),
                    platform: platform.clone(),
                    action: SyncAction::Skipped as i32,
                    success: true,
                    error_message: String::new(),
                });
            }
        }
        
        Ok(Response::new(SyncRedeemsResponse {
            results,
            created_count,
            updated_count,
            deleted_count,
            error_count,
        }))
    }
    async fn get_sync_status(&self, request: Request<GetSyncStatusRequest>) -> Result<Response<GetSyncStatusResponse>, Status> {
        let req = request.into_inner();
        debug!("Getting sync status for platforms: {:?}", req.platforms);
        
        let mut platform_statuses = Vec::new();
        
        let platforms = if req.platforms.is_empty() {
            // TODO: Get all platforms
            vec!["twitch".to_string()]
        } else {
            req.platforms
        };
        
        for platform in platforms {
            let redeems = self.redeem_repo.list_redeems(&platform).await
                .map_err(|e| Status::internal(format!("Failed to list redeems: {}", e)))?;
            
            let local_count = redeems.len() as i32;
            let synced_count = redeems.iter().filter(|r| r.is_active).count() as i32; // Mock synced as active
            let platform_count = synced_count; // Mock platform count
            let out_of_sync_count = local_count - synced_count;
            
            platform_statuses.push(PlatformSyncStatus {
                platform,
                local_count,
                platform_count,
                synced_count,
                out_of_sync_count,
                last_sync: Some(prost_types::Timestamp {
                    seconds: Utc::now().timestamp() - 3600, // 1 hour ago
                    nanos: 0,
                }),
                sync_enabled: true,
            });
        }
        
        Ok(Response::new(GetSyncStatusResponse {
            platforms: platform_statuses,
            last_full_sync: Some(prost_types::Timestamp {
                seconds: Utc::now().timestamp() - 7200, // 2 hours ago
                nanos: 0,
            }),
        }))
    }
    async fn execute_redeem(&self, request: Request<ExecuteRedeemRequest>) -> Result<Response<ExecuteRedeemResponse>, Status> {
        let req = request.into_inner();
        let redeem_id = Uuid::parse_str(&req.redeem_id)
            .map_err(|e| Status::invalid_argument(format!("Invalid redeem ID: {}", e)))?;
        let user_id = Uuid::parse_str(&req.user_id)
            .map_err(|e| Status::invalid_argument(format!("Invalid user ID: {}", e)))?;
        
        debug!("Executing redeem: {} for user: {}", redeem_id, user_id);
        
        // Get the redeem
        let rd = self.redeem_repo.get_redeem_by_id(redeem_id).await
            .map_err(|e| Status::internal(format!("Failed to get redeem: {}", e)))?;
        
        let rd = match rd {
            Some(r) => r,
            None => {
                return Ok(Response::new(ExecuteRedeemResponse {
                    executed: false,
                    response: String::new(),
                    redemption_id: String::new(),
                    error_message: "Redeem not found".to_string(),
                }));
            }
        };
        
        // Check if redeem is active
        if !rd.is_active {
            return Ok(Response::new(ExecuteRedeemResponse {
                executed: false,
                response: String::new(),
                redemption_id: String::new(),
                error_message: "Redeem is disabled".to_string(),
            }));
        }
        
        // TODO: Actually execute the redeem through the redeem service
        // For now, just create a usage record
        let redemption_id = Uuid::new_v4();
        let usage = maowbot_common::models::RedeemUsage {
            usage_id: redemption_id,
            redeem_id,
            user_id,
            used_at: Utc::now(),
            channel: req.context.get("channel").cloned(),
            usage_data: Some(serde_json::json!({
                "user_input": req.user_input,
                "platform_user_id": req.platform_user_id,
                "status": "fulfilled",
                "response": format!("Redeemed '{}'", rd.reward_name),
            })),
        };
        
        self.redeem_usage_repo.insert_usage(&usage).await
            .map_err(|e| Status::internal(format!("Failed to record usage: {}", e)))?;
        
        Ok(Response::new(ExecuteRedeemResponse {
            executed: true,
            response: format!("Successfully redeemed '{}'", rd.reward_name),
            redemption_id: redemption_id.to_string(),
            error_message: String::new(),
        }))
    }
    async fn test_redeem(&self, request: Request<TestRedeemRequest>) -> Result<Response<TestRedeemResponse>, Status> {
        let req = request.into_inner();
        let redeem_id = Uuid::parse_str(&req.redeem_id)
            .map_err(|e| Status::invalid_argument(format!("Invalid redeem ID: {}", e)))?;
        
        debug!("Testing redeem: {}", redeem_id);
        
        // Get the redeem
        let rd = self.redeem_repo.get_redeem_by_id(redeem_id).await
            .map_err(|e| Status::internal(format!("Failed to get redeem: {}", e)))?;
        
        let rd = match rd {
            Some(r) => r,
            None => {
                return Ok(Response::new(TestRedeemResponse {
                    would_execute: false,
                    expected_response: String::new(),
                    triggered_actions: vec![],
                    error_message: "Redeem not found".to_string(),
                }));
            }
        };
        
        // Check if redeem would execute
        let would_execute = rd.is_active;
        let mut triggered_actions = Vec::new();
        
        if would_execute {
            // List what would be triggered
            if rd.plugin_name.is_some() && rd.command_name.is_some() {
                triggered_actions.push(format!("Plugin: {}, Command: {}", 
                    rd.plugin_name.as_ref().unwrap(), 
                    rd.command_name.as_ref().unwrap()));
            }
            
            // Check for OSC triggers
            // TODO: Query OSC triggers for this redeem
            triggered_actions.push("Check OSC triggers (not implemented)".to_string());
        }
        
        Ok(Response::new(TestRedeemResponse {
            would_execute,
            expected_response: if would_execute {
                format!("Redeem '{}' would execute with input: {}", rd.reward_name, req.test_input)
            } else {
                String::new()
            },
            triggered_actions,
            error_message: if !would_execute {
                "Redeem is disabled".to_string()
            } else {
                String::new()
            },
        }))
    }
    async fn get_redeem_usage(&self, request: Request<GetRedeemUsageRequest>) -> Result<Response<GetRedeemUsageResponse>, Status> {
        let req = request.into_inner();
        debug!("Getting redeem usage");
        
        let mut usage_data = Vec::new();
        
        // If redeem_id is specified, get usage for that redeem
        if !req.redeem_id.is_empty() {
            let redeem_id = Uuid::parse_str(&req.redeem_id)
                .map_err(|e| Status::invalid_argument(format!("Invalid redeem ID: {}", e)))?;
            
            // Get the redeem first
            let rd = self.redeem_repo.get_redeem_by_id(redeem_id).await
                .map_err(|e| Status::internal(format!("Failed to get redeem: {}", e)))?;
            
            if let Some(rd) = rd {
                // Get usage for this redeem
                let usages = self.redeem_usage_repo.list_usage_for_redeem(redeem_id, 100).await
                    .map_err(|e| Status::internal(format!("Failed to get usage data: {}", e)))?;
                
                let redemption_count = usages.len() as i64;
                let unique_users = usages.iter()
                    .map(|u| u.user_id)
                    .collect::<std::collections::HashSet<_>>()
                    .len() as i64;
                let total_cost = rd.cost as i64 * redemption_count;
                
                let mut usage_by_status = HashMap::new();
                for usage in &usages {
                    if let Some(data) = &usage.usage_data {
                        let status = data.get("status").and_then(|v| v.as_str()).unwrap_or("unknown");
                        *usage_by_status.entry(status.to_string()).or_insert(0) += 1;
                    }
                }
                
                usage_data.push(RedeemUsageData {
                    redeem_id: redeem_id.to_string(),
                    redeem_name: rd.reward_name,
                    timestamp: Some(prost_types::Timestamp {
                        seconds: Utc::now().timestamp(),
                        nanos: 0,
                    }),
                    redemption_count,
                    unique_users,
                    total_cost,
                    usage_by_status,
                });
            }
        } else if !req.platform.is_empty() {
            // Get all redeems for platform and their usage
            let redeems = self.redeem_repo.list_redeems(&req.platform).await
                .map_err(|e| Status::internal(format!("Failed to list redeems: {}", e)))?;
            
            for rd in redeems {
                // Get limited usage data for each redeem
                let usages = self.redeem_usage_repo.list_usage_for_redeem(rd.redeem_id, 10).await
                    .map_err(|e| Status::internal(format!("Failed to get usage data: {}", e)))?;
                
                let redemption_count = usages.len() as i64;
                let unique_users = usages.iter()
                    .map(|u| u.user_id)
                    .collect::<std::collections::HashSet<_>>()
                    .len() as i64;
                let total_cost = rd.cost as i64 * redemption_count;
                
                usage_data.push(RedeemUsageData {
                    redeem_id: rd.redeem_id.to_string(),
                    redeem_name: rd.reward_name,
                    timestamp: Some(prost_types::Timestamp {
                        seconds: Utc::now().timestamp(),
                        nanos: 0,
                    }),
                    redemption_count,
                    unique_users,
                    total_cost,
                    usage_by_status: HashMap::new(),
                });
            }
        }
        
        // Calculate summary
        let total_redemptions = usage_data.iter().map(|d| d.redemption_count).sum();
        let total_unique_users = usage_data.iter().map(|d| d.unique_users).sum();
        let total_points_spent = usage_data.iter().map(|d| d.total_cost).sum();
        let most_redeemed = usage_data.iter()
            .max_by_key(|d| d.redemption_count)
            .map(|d| d.redeem_name.clone())
            .unwrap_or_default();
        let highest_cost_redeemed = usage_data.iter()
            .max_by_key(|d| d.total_cost)
            .map(|d| d.redeem_name.clone())
            .unwrap_or_default();
        
        let fulfilled_count = usage_data.iter()
            .flat_map(|d| d.usage_by_status.get("fulfilled").copied())
            .sum::<i64>() as f32;
        let fulfillment_rate = if total_redemptions > 0 {
            fulfilled_count / total_redemptions as f32
        } else {
            0.0
        };
        
        let summary = RedeemUsageSummary {
            total_redemptions,
            total_unique_users,
            total_points_spent,
            most_redeemed,
            highest_cost_redeemed,
            average_redemptions_per_day: 0.0, // TODO: Calculate based on time range
            fulfillment_rate,
        };
        
        Ok(Response::new(GetRedeemUsageResponse {
            usage: usage_data,
            summary: Some(summary),
        }))
    }
    type StreamRedeemEventsStream = tonic::codec::Streaming<RedeemEvent>;
    async fn stream_redeem_events(&self, _: Request<StreamRedeemEventsRequest>) -> Result<Response<Self::StreamRedeemEventsStream>, Status> {
        // TODO: Implement streaming of redeem events
        Err(Status::unimplemented("Redeem event streaming not yet implemented"))
    }
}