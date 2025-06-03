use tonic::{Request, Response, Status};
use prost_types;
use maowbot_proto::maowbot::{
    common::{User, PlatformIdentity, UserAnalysis, Platform, PageRequest, PageResponse},
    services::{
        user_service_server::UserService,
        MergeStrategy,
        *,
    },
};
use maowbot_core::repositories::postgres::{
    user::UserRepository,
    user_analysis::PostgresUserAnalysisRepository,
    platform_identity::PlatformIdentityRepository,
};
use maowbot_common::{
    models::{
        user as user_models,
        platform::PlatformIdentity as PlatformIdentityModel,
        user_analysis::UserAnalysis as UserAnalysisModel,
    },
    traits::repository_traits::{UserAnalysisRepository, UserRepo, PlatformIdentityRepo},
};
use std::sync::Arc;
use std::str::FromStr;
use uuid::Uuid;
use chrono::Utc;
use tracing::{info, error, debug};

pub struct UserServiceImpl {
    user_repo: Arc<UserRepository>,
    analysis_repo: Arc<PostgresUserAnalysisRepository>,
    platform_identity_repo: Arc<PlatformIdentityRepository>,
}

impl UserServiceImpl {
    pub fn new(
        user_repo: Arc<UserRepository>,
        analysis_repo: Arc<PostgresUserAnalysisRepository>,
        platform_identity_repo: Arc<PlatformIdentityRepository>,
    ) -> Self {
        Self {
            user_repo,
            analysis_repo,
            platform_identity_repo,
        }
    }
    
    // Helper to convert from internal model to proto
    fn user_to_proto(user: &user_models::User) -> User {
        User {
            user_id: user.user_id.to_string(),
            global_username: user.global_username.clone().unwrap_or_default(),
            created_at: Some(prost_types::Timestamp {
                seconds: user.created_at.timestamp(),
                nanos: user.created_at.timestamp_subsec_nanos() as i32,
            }),
            last_seen: Some(prost_types::Timestamp {
                seconds: user.last_seen.timestamp(),
                nanos: user.last_seen.timestamp_subsec_nanos() as i32,
            }),
            is_active: user.is_active,
        }
    }
    
    // Helper to convert platform identity
    fn platform_identity_to_proto(identity: &PlatformIdentityModel) -> maowbot_proto::maowbot::common::PlatformIdentity {
        maowbot_proto::maowbot::common::PlatformIdentity {
            platform_identity_id: identity.platform_identity_id.to_string(),
            user_id: identity.user_id.to_string(),
            platform: match identity.platform {
                maowbot_common::models::platform::Platform::TwitchIRC => Platform::TwitchIrc as i32,
                maowbot_common::models::platform::Platform::TwitchEventSub => Platform::TwitchEventsub as i32,
                maowbot_common::models::platform::Platform::Discord => Platform::Discord as i32,
                maowbot_common::models::platform::Platform::VRChat => Platform::Vrchat as i32,
                _ => Platform::Unknown as i32,
            },
            platform_user_id: identity.platform_user_id.clone(),
            platform_username: identity.platform_username.clone(),
            platform_display_name: identity.platform_display_name.clone().unwrap_or_default(),
            platform_roles: identity.platform_roles.clone(),
            platform_data: None, // TODO: Convert platform_data JSON to Any
            created_at: Some(prost_types::Timestamp {
                seconds: identity.created_at.timestamp(),
                nanos: identity.created_at.timestamp_subsec_nanos() as i32,
            }),
            last_updated: Some(prost_types::Timestamp {
                seconds: identity.last_updated.timestamp(),
                nanos: identity.last_updated.timestamp_subsec_nanos() as i32,
            }),
        }
    }
    
    // Helper to convert user analysis
    fn user_analysis_to_proto(analysis: &UserAnalysisModel) -> maowbot_proto::maowbot::common::UserAnalysis {
        maowbot_proto::maowbot::common::UserAnalysis {
            user_analysis_id: analysis.user_analysis_id.to_string(),
            user_id: analysis.user_id.to_string(),
            spam_score: analysis.spam_score,
            intelligibility_score: analysis.intelligibility_score,
            quality_score: analysis.quality_score,
            horni_score: analysis.horni_score,
            ai_notes: analysis.ai_notes.clone().unwrap_or_default(),
            moderator_notes: analysis.moderator_notes.clone().unwrap_or_default(),
            created_at: Some(prost_types::Timestamp {
                seconds: analysis.created_at.timestamp(),
                nanos: analysis.created_at.timestamp_subsec_nanos() as i32,
            }),
            updated_at: Some(prost_types::Timestamp {
                seconds: analysis.updated_at.timestamp(),
                nanos: analysis.updated_at.timestamp_subsec_nanos() as i32,
            }),
        }
    }
}

#[tonic::async_trait]
impl UserService for UserServiceImpl {
    async fn create_user(
        &self,
        request: Request<CreateUserRequest>,
    ) -> Result<Response<CreateUserResponse>, Status> {
        let req = request.into_inner();
        info!("Creating user with display_name: {}", req.display_name);
        
        let user_id = if req.user_id.is_empty() {
            Uuid::new_v4()
        } else {
            Uuid::parse_str(&req.user_id)
                .map_err(|e| Status::invalid_argument(format!("Invalid user_id: {}", e)))?
        };
        
        let new_user = user_models::User {
            user_id,
            global_username: Some(req.display_name.clone()),
            created_at: Utc::now(),
            last_seen: Utc::now(),
            is_active: req.is_active,
        };
        
        self.user_repo
            .create(&new_user)
            .await
            .map_err(|e| Status::internal(format!("Failed to create user: {}", e)))?;
        
        Ok(Response::new(CreateUserResponse {
            user: Some(Self::user_to_proto(&new_user)),
        }))
    }
    
    async fn get_user(
        &self,
        request: Request<GetUserRequest>,
    ) -> Result<Response<GetUserResponse>, Status> {
        let req = request.into_inner();
        debug!("Getting user: {}", req.user_id);
        
        let user_id = Uuid::parse_str(&req.user_id)
            .map_err(|e| Status::invalid_argument(format!("Invalid user_id: {}", e)))?;
        
        let user = self.user_repo
            .get(user_id)
            .await
            .map_err(|e| Status::internal(format!("Failed to get user: {}", e)))?
            .ok_or_else(|| Status::not_found("User not found"))?;
        
        let mut response = GetUserResponse {
            user: Some(Self::user_to_proto(&user)),
            identities: vec![],
            analysis: None,
        };
        
        // Get platform identities if requested
        if req.include_identities {
            if let Ok(identities) = self.platform_identity_repo.get_all_for_user(user_id).await {
                response.identities = identities.into_iter()
                    .map(|i| Self::platform_identity_to_proto(&i))
                    .collect();
            }
        }
        
        // Get analysis if requested
        if req.include_analysis {
            if let Ok(Some(analysis)) = self.analysis_repo.get_analysis(user_id).await {
                response.analysis = Some(Self::user_analysis_to_proto(&analysis));
            }
        }
        
        Ok(Response::new(response))
    }
    
    async fn update_user(
        &self,
        request: Request<UpdateUserRequest>,
    ) -> Result<Response<UpdateUserResponse>, Status> {
        let req = request.into_inner();
        info!("Updating user: {}", req.user_id);
        
        let user_id = Uuid::parse_str(&req.user_id)
            .map_err(|e| Status::invalid_argument(format!("Invalid user_id: {}", e)))?;
        
        // Get existing user
        let mut user = self.user_repo
            .get(user_id)
            .await
            .map_err(|e| Status::internal(format!("Failed to get user: {}", e)))?
            .ok_or_else(|| Status::not_found("User not found"))?;
        
        // Apply updates based on field mask
        if let Some(update_mask) = req.update_mask {
            for path in &update_mask.paths {
                match path.as_str() {
                    "global_username" => {
                        if let Some(ref new_user) = req.user {
                            user.global_username = Some(new_user.global_username.clone());
                        }
                    }
                    "is_active" => {
                        if let Some(ref new_user) = req.user {
                            user.is_active = new_user.is_active;
                        }
                    }
                    _ => {
                        return Err(Status::invalid_argument(format!("Unknown field path: {}", path)));
                    }
                }
            }
        } else if let Some(ref new_user) = req.user {
            // No field mask, update all fields
            user.global_username = Some(new_user.global_username.clone());
            user.is_active = new_user.is_active;
        }
        
        user.last_seen = Utc::now();
        
        self.user_repo
            .update(&user)
            .await
            .map_err(|e| Status::internal(format!("Failed to update user: {}", e)))?;
        
        Ok(Response::new(UpdateUserResponse {
            user: Some(Self::user_to_proto(&user)),
        }))
    }
    
    async fn delete_user(
        &self,
        request: Request<DeleteUserRequest>,
    ) -> Result<Response<()>, Status> {
        let req = request.into_inner();
        info!("Deleting user: {}", req.user_id);
        
        let user_id = Uuid::parse_str(&req.user_id)
            .map_err(|e| Status::invalid_argument(format!("Invalid user_id: {}", e)))?;
        
        if req.hard_delete {
            self.user_repo
                .delete(user_id)
                .await
                .map_err(|e| Status::internal(format!("Failed to delete user: {}", e)))?;
        } else {
            // Soft delete - just mark as inactive
            if let Some(mut user) = self.user_repo.get(user_id).await
                .map_err(|e| Status::internal(format!("Failed to get user: {}", e)))? {
                user.is_active = false;
                self.user_repo
                    .update(&user)
                    .await
                    .map_err(|e| Status::internal(format!("Failed to update user: {}", e)))?;
            }
        }
        
        Ok(Response::new(()))
    }
    
    async fn list_users(
        &self,
        request: Request<ListUsersRequest>,
    ) -> Result<Response<ListUsersResponse>, Status> {
        let req = request.into_inner();
        debug!("Listing users");
        
        // For now, return all users that are active
        // TODO: Implement proper pagination and filtering
        let all_users = self.user_repo
            .list_all()
            .await
            .map_err(|e| Status::internal(format!("Failed to list users: {}", e)))?;
        
        let users: Vec<user_models::User> = all_users.into_iter()
            .filter(|u| u.is_active)
            .collect();
        
        let proto_users: Vec<User> = users.into_iter()
            .map(|u| Self::user_to_proto(&u))
            .collect();
        
        Ok(Response::new(ListUsersResponse {
            users: proto_users,
            page: Some(PageResponse {
                next_page_token: String::new(),
                total_count: 0, // TODO: Get actual count
            }),
        }))
    }
    
    async fn search_users(
        &self,
        request: Request<SearchUsersRequest>,
    ) -> Result<Response<SearchUsersResponse>, Status> {
        let req = request.into_inner();
        info!("Searching users with query: {}", req.query);
        
        // Simple search by username - using list_all and filtering
        let all_users = self.user_repo
            .list_all()
            .await
            .map_err(|e| Status::internal(format!("Failed to search users: {}", e)))?;
        
        let query_lower = req.query.to_lowercase();
        let users: Vec<user_models::User> = all_users.into_iter()
            .filter(|u| u.global_username.as_ref()
                .map(|n| n.to_lowercase().contains(&query_lower))
                .unwrap_or(false))
            .collect();
        
        let results: Vec<UserSearchResult> = users.into_iter()
            .map(|u| UserSearchResult {
                user: Some(Self::user_to_proto(&u)),
                matched_identities: vec![],
                relevance_score: 1.0, // TODO: Calculate actual relevance
            })
            .collect();
        
        Ok(Response::new(SearchUsersResponse {
            results,
            page: Some(PageResponse {
                next_page_token: String::new(),
                total_count: 0,
            }),
        }))
    }
    
    async fn find_user_by_name(
        &self,
        request: Request<FindUserByNameRequest>,
    ) -> Result<Response<FindUserByNameResponse>, Status> {
        let req = request.into_inner();
        debug!("Finding user by name: {}", req.name);
        
        let users = if req.exact_match {
            // Exact match
            if let Some(user) = self.user_repo
                .get_by_global_username(&req.name)
                .await
                .map_err(|e| Status::internal(format!("Failed to find user: {}", e)))? {
                vec![user]
            } else {
                vec![]
            }
        } else {
            // Fuzzy search - using list_all and filtering
            let all_users = self.user_repo
                .list_all()
                .await
                .map_err(|e| Status::internal(format!("Failed to search users: {}", e)))?;
            
            let name_lower = req.name.to_lowercase();
            all_users.into_iter()
                .filter(|u| u.global_username.as_ref()
                    .map(|n| n.to_lowercase().contains(&name_lower))
                    .unwrap_or(false))
                .collect()
        };
        
        let proto_users: Vec<User> = users.into_iter()
            .map(|u| Self::user_to_proto(&u))
            .collect();
        
        Ok(Response::new(FindUserByNameResponse {
            users: proto_users,
        }))
    }
    
    async fn batch_get_users(
        &self,
        request: Request<BatchGetUsersRequest>,
    ) -> Result<Response<BatchGetUsersResponse>, Status> {
        let req = request.into_inner();
        debug!("Batch getting {} users", req.user_ids.len());
        
        let mut users = Vec::new();
        let mut not_found = Vec::new();
        
        for user_id_str in &req.user_ids {
            let user_id = match Uuid::parse_str(user_id_str) {
                Ok(id) => id,
                Err(_) => {
                    not_found.push(user_id_str.clone());
                    continue;
                }
            };
            
            match self.user_repo.get(user_id).await {
                Ok(Some(user)) => users.push(Self::user_to_proto(&user)),
                Ok(None) => not_found.push(user_id_str.clone()),
                Err(e) => {
                    error!("Error fetching user {}: {}", user_id, e);
                    not_found.push(user_id_str.clone());
                }
            }
        }
        
        // Convert to GetUserResponse format
        let user_responses: Vec<GetUserResponse> = users.into_iter()
            .map(|user| GetUserResponse {
                user: Some(user),
                identities: vec![],
                analysis: None,
            })
            .collect();
        
        Ok(Response::new(BatchGetUsersResponse {
            users: user_responses,
            not_found_ids: not_found,
        }))
    }
    
    async fn merge_users(
        &self,
        request: Request<MergeUsersRequest>,
    ) -> Result<Response<MergeUsersResponse>, Status> {
        let req = request.into_inner();
        info!("Merging user {} into {}", req.source_user_id, req.target_user_id);
        
        let from_user_id = Uuid::parse_str(&req.source_user_id)
            .map_err(|e| Status::invalid_argument(format!("Invalid source_user_id: {}", e)))?;
        let to_user_id = Uuid::parse_str(&req.target_user_id)
            .map_err(|e| Status::invalid_argument(format!("Invalid target_user_id: {}", e)))?;
        
        // Verify both users exist
        let from_user = self.user_repo.get(from_user_id).await
            .map_err(|e| Status::internal(format!("Failed to get from_user: {}", e)))?
            .ok_or_else(|| Status::not_found("From user not found"))?;
        
        let to_user = self.user_repo.get(to_user_id).await
            .map_err(|e| Status::internal(format!("Failed to get to_user: {}", e)))?
            .ok_or_else(|| Status::not_found("To user not found"))?;
        
        // Get all platform identities from the source user
        let from_identities = self.platform_identity_repo.get_all_for_user(from_user_id).await
            .map_err(|e| Status::internal(format!("Failed to get identities: {}", e)))?;
        
        // Update each identity to point to the target user
        let mut merged_identities = 0;
        for mut identity in from_identities {
            identity.user_id = to_user_id;
            identity.last_updated = Utc::now();
            
            if let Err(e) = self.platform_identity_repo.update(&identity).await {
                error!("Failed to update identity {}: {}", identity.platform_identity_id, e);
            } else {
                merged_identities += 1;
            }
        }
        
        // Delete the source user based on strategy
        if req.strategy == MergeStrategy::KeepTarget as i32 {
            self.user_repo.delete(from_user_id).await
                .map_err(|e| Status::internal(format!("Failed to delete source user: {}", e)))?;
        }
        
        // Get the identity IDs that were merged
        let merged_identity_ids = self.platform_identity_repo.get_all_for_user(to_user_id).await
            .map_err(|e| Status::internal(format!("Failed to get merged identities: {}", e)))?
            .into_iter()
            .map(|i| i.platform_identity_id.to_string())
            .collect();
        
        Ok(Response::new(MergeUsersResponse {
            merged_user: Some(Self::user_to_proto(&to_user)),
            merged_identity_ids,
        }))
    }
    
    async fn get_platform_identities(
        &self,
        request: Request<GetPlatformIdentitiesRequest>,
    ) -> Result<Response<GetPlatformIdentitiesResponse>, Status> {
        let req = request.into_inner();
        debug!("Getting platform identities for user: {}", req.user_id);
        
        let user_id = Uuid::parse_str(&req.user_id)
            .map_err(|e| Status::invalid_argument(format!("Invalid user_id: {}", e)))?;
        
        let identities = self.platform_identity_repo.get_all_for_user(user_id).await
            .map_err(|e| Status::internal(format!("Failed to get identities: {}", e)))?;
        
        let filtered_identities = if req.platforms.is_empty() {
            identities
        } else {
            // Filter by requested platforms
            let requested_platforms: Vec<Platform> = req.platforms.iter()
                .filter_map(|&p| Platform::try_from(p).ok())
                .collect();
            
            identities.into_iter()
                .filter(|i| {
                    let proto_platform = match i.platform {
                        maowbot_common::models::platform::Platform::TwitchIRC => Platform::TwitchIrc,
                        maowbot_common::models::platform::Platform::TwitchEventSub => Platform::TwitchEventsub,
                        maowbot_common::models::platform::Platform::Discord => Platform::Discord,
                        maowbot_common::models::platform::Platform::VRChat => Platform::Vrchat,
                        _ => Platform::Unknown,
                    };
                    requested_platforms.contains(&proto_platform)
                })
                .collect()
        };
        
        let proto_identities: Vec<PlatformIdentity> = filtered_identities.into_iter()
            .map(|i| Self::platform_identity_to_proto(&i))
            .collect();
        
        Ok(Response::new(GetPlatformIdentitiesResponse {
            identities: proto_identities,
        }))
    }
    
    async fn add_platform_identity(
        &self,
        request: Request<AddPlatformIdentityRequest>,
    ) -> Result<Response<AddPlatformIdentityResponse>, Status> {
        let req = request.into_inner();
        let user_id = Uuid::parse_str(&req.user_id)
            .map_err(|e| Status::invalid_argument(format!("Invalid user_id: {}", e)))?;
        
        let identity_proto = req.identity.ok_or_else(|| Status::invalid_argument("Missing identity"))?;
        
        info!("Adding platform identity for user: {}", req.user_id);
        
        // Verify user exists
        self.user_repo.get(user_id).await
            .map_err(|e| Status::internal(format!("Failed to verify user: {}", e)))?
            .ok_or_else(|| Status::not_found("User not found"))?;
        
        let platform = match Platform::try_from(identity_proto.platform) {
            Ok(Platform::TwitchIrc) => maowbot_common::models::platform::Platform::TwitchIRC,
            Ok(Platform::TwitchEventsub) => maowbot_common::models::platform::Platform::TwitchEventSub,
            Ok(Platform::Discord) => maowbot_common::models::platform::Platform::Discord,
            Ok(Platform::Vrchat) => maowbot_common::models::platform::Platform::VRChat,
            _ => return Err(Status::invalid_argument("Invalid platform")),
        };
        
        let new_identity = PlatformIdentityModel {
            platform_identity_id: Uuid::new_v4(),
            user_id,
            platform,
            platform_user_id: identity_proto.platform_user_id,
            platform_username: identity_proto.platform_username,
            platform_display_name: Some(identity_proto.platform_display_name),
            platform_roles: identity_proto.platform_roles,
            platform_data: serde_json::Value::Object(serde_json::Map::new()),
            created_at: Utc::now(),
            last_updated: Utc::now(),
        };
        
        self.platform_identity_repo.create(&new_identity).await
            .map_err(|e| Status::internal(format!("Failed to create identity: {}", e)))?;
        
        Ok(Response::new(AddPlatformIdentityResponse {
            identity: Some(Self::platform_identity_to_proto(&new_identity)),
        }))
    }
    
    async fn update_platform_identity(
        &self,
        request: Request<UpdatePlatformIdentityRequest>,
    ) -> Result<Response<UpdatePlatformIdentityResponse>, Status> {
        let req = request.into_inner();
        info!("Updating platform identity: {}", req.identity_id);
        
        let identity_id = Uuid::parse_str(&req.identity_id)
            .map_err(|e| Status::invalid_argument(format!("Invalid identity_id: {}", e)))?;
        
        let mut identity = self.platform_identity_repo.get(identity_id).await
            .map_err(|e| Status::internal(format!("Failed to get identity: {}", e)))?
            .ok_or_else(|| Status::not_found("Platform identity not found"))?;
        
        // Apply updates based on field mask
        if let Some(update_mask) = req.update_mask {
            for path in &update_mask.paths {
                match path.as_str() {
                    "platform_username" => {
                        if let Some(ref updates) = req.identity {
                            identity.platform_username = updates.platform_username.clone();
                        }
                    }
                    "platform_display_name" => {
                        if let Some(ref updates) = req.identity {
                            identity.platform_display_name = Some(updates.platform_display_name.clone());
                        }
                    }
                    "platform_roles" => {
                        if let Some(ref updates) = req.identity {
                            identity.platform_roles = updates.platform_roles.clone();
                        }
                    }
                    _ => {
                        return Err(Status::invalid_argument(format!("Unknown field path: {}", path)));
                    }
                }
            }
        } else if let Some(ref updates) = req.identity {
            // No field mask, update all allowed fields
            identity.platform_username = updates.platform_username.clone();
            identity.platform_display_name = Some(updates.platform_display_name.clone());
            identity.platform_roles = updates.platform_roles.clone();
        }
        
        identity.last_updated = Utc::now();
        
        self.platform_identity_repo.update(&identity).await
            .map_err(|e| Status::internal(format!("Failed to update identity: {}", e)))?;
        
        Ok(Response::new(UpdatePlatformIdentityResponse {
            identity: Some(Self::platform_identity_to_proto(&identity)),
        }))
    }
    
    async fn remove_platform_identity(
        &self,
        request: Request<RemovePlatformIdentityRequest>,
    ) -> Result<Response<()>, Status> {
        let req = request.into_inner();
        info!("Removing platform identity: {}", req.identity_id);
        
        let identity_id = Uuid::parse_str(&req.identity_id)
            .map_err(|e| Status::invalid_argument(format!("Invalid identity_id: {}", e)))?;
        
        self.platform_identity_repo.delete(identity_id).await
            .map_err(|e| Status::internal(format!("Failed to delete identity: {}", e)))?;
        
        Ok(Response::new(()))
    }
    
    async fn add_role_to_identity(
        &self,
        request: Request<AddRoleToIdentityRequest>,
    ) -> Result<Response<()>, Status> {
        let req = request.into_inner();
        info!("Adding role {} to user {} on platform {}", req.role, req.user_id, req.platform);
        
        let user_id = Uuid::parse_str(&req.user_id)
            .map_err(|e| Status::invalid_argument(format!("Invalid user_id: {}", e)))?;
        
        let platform = maowbot_common::models::platform::Platform::from_str(&req.platform)
            .map_err(|_| Status::invalid_argument("Invalid platform"))?;
        
        let mut identity = self.platform_identity_repo.get_by_user_and_platform(user_id, &platform).await
            .map_err(|e| Status::internal(format!("Failed to get identity: {}", e)))?
            .ok_or_else(|| Status::not_found("Platform identity not found"))?;
        
        // Add role if not already present
        if !identity.platform_roles.contains(&req.role) {
            identity.platform_roles.push(req.role);
            identity.last_updated = Utc::now();
            
            self.platform_identity_repo.update(&identity).await
                .map_err(|e| Status::internal(format!("Failed to update identity: {}", e)))?;
        }
        
        Ok(Response::new(()))
    }
    
    async fn remove_role_from_identity(
        &self,
        request: Request<RemoveRoleFromIdentityRequest>,
    ) -> Result<Response<()>, Status> {
        let req = request.into_inner();
        info!("Removing role {} from user {} on platform {}", req.role, req.user_id, req.platform);
        
        let user_id = Uuid::parse_str(&req.user_id)
            .map_err(|e| Status::invalid_argument(format!("Invalid user_id: {}", e)))?;
        
        let platform = maowbot_common::models::platform::Platform::from_str(&req.platform)
            .map_err(|_| Status::invalid_argument("Invalid platform"))?;
        
        let mut identity = self.platform_identity_repo.get_by_user_and_platform(user_id, &platform).await
            .map_err(|e| Status::internal(format!("Failed to get identity: {}", e)))?
            .ok_or_else(|| Status::not_found("Platform identity not found"))?;
        
        // Remove role if present
        if let Some(pos) = identity.platform_roles.iter().position(|r| r == &req.role) {
            identity.platform_roles.remove(pos);
            identity.last_updated = Utc::now();
            
            self.platform_identity_repo.update(&identity).await
                .map_err(|e| Status::internal(format!("Failed to update identity: {}", e)))?;
        }
        
        Ok(Response::new(()))
    }
    
    async fn get_user_analysis(
        &self,
        request: Request<GetUserAnalysisRequest>,
    ) -> Result<Response<GetUserAnalysisResponse>, Status> {
        let req = request.into_inner();
        debug!("Getting user analysis for: {}", req.user_id);
        
        let user_id = Uuid::parse_str(&req.user_id)
            .map_err(|e| Status::invalid_argument(format!("Invalid user_id: {}", e)))?;
        
        let analysis = self.analysis_repo.get_analysis(user_id).await
            .map_err(|e| Status::internal(format!("Failed to get analysis: {}", e)))?
            .ok_or_else(|| Status::not_found("User analysis not found"))?;
        
        Ok(Response::new(GetUserAnalysisResponse {
            analysis: Some(Self::user_analysis_to_proto(&analysis)),
            history: vec![], // TODO: Implement analysis history tracking
        }))
    }
    
    async fn update_user_analysis(
        &self,
        request: Request<UpdateUserAnalysisRequest>,
    ) -> Result<Response<UpdateUserAnalysisResponse>, Status> {
        let req = request.into_inner();
        let analysis_proto = req.analysis.ok_or_else(|| Status::invalid_argument("Missing analysis"))?;
        
        info!("Updating user analysis for: {}", analysis_proto.user_id);
        
        let user_id = Uuid::parse_str(&analysis_proto.user_id)
            .map_err(|e| Status::invalid_argument(format!("Invalid user_id: {}", e)))?;
        
        // Get existing analysis or create new one
        let mut analysis = match self.analysis_repo.get_analysis(user_id).await
            .map_err(|e| Status::internal(format!("Failed to get analysis: {}", e)))? {
            Some(existing) => existing,
            None => {
                // Create new analysis
                UserAnalysisModel {
                    user_analysis_id: Uuid::new_v4(),
                    user_id,
                    spam_score: 0.0,
                    intelligibility_score: 0.0,
                    quality_score: 0.0,
                    horni_score: 0.0,
                    ai_notes: None,
                    moderator_notes: None,
                    created_at: Utc::now(),
                    updated_at: Utc::now(),
                }
            }
        };
        
        // Apply updates based on field mask
        if let Some(update_mask) = req.update_mask {
            for path in &update_mask.paths {
                match path.as_str() {
                    "spam_score" => analysis.spam_score = analysis_proto.spam_score,
                    "intelligibility_score" => analysis.intelligibility_score = analysis_proto.intelligibility_score,
                    "quality_score" => analysis.quality_score = analysis_proto.quality_score,
                    "horni_score" => analysis.horni_score = analysis_proto.horni_score,
                    "ai_notes" => analysis.ai_notes = Some(analysis_proto.ai_notes.clone()),
                    "moderator_notes" => analysis.moderator_notes = Some(analysis_proto.moderator_notes.clone()),
                    _ => return Err(Status::invalid_argument(format!("Unknown field path: {}", path))),
                }
            }
        } else {
            // No field mask, update all fields
            analysis.spam_score = analysis_proto.spam_score;
            analysis.intelligibility_score = analysis_proto.intelligibility_score;
            analysis.quality_score = analysis_proto.quality_score;
            analysis.horni_score = analysis_proto.horni_score;
            if !analysis_proto.ai_notes.is_empty() {
                analysis.ai_notes = Some(analysis_proto.ai_notes.clone());
            }
            if !analysis_proto.moderator_notes.is_empty() {
                analysis.moderator_notes = Some(analysis_proto.moderator_notes.clone());
            }
        }
        
        analysis.updated_at = Utc::now();
        
        // Save the analysis
        if self.analysis_repo.get_analysis(user_id).await
            .map_err(|e| Status::internal(format!("Failed to check analysis: {}", e)))?
            .is_some() {
            self.analysis_repo.update_analysis(&analysis).await
                .map_err(|e| Status::internal(format!("Failed to update analysis: {}", e)))?;
        } else {
            self.analysis_repo.create_analysis(&analysis).await
                .map_err(|e| Status::internal(format!("Failed to create analysis: {}", e)))?;
        }
        
        Ok(Response::new(UpdateUserAnalysisResponse {
            analysis: Some(Self::user_analysis_to_proto(&analysis)),
        }))
    }
    
    async fn append_moderator_note(
        &self,
        request: Request<AppendModeratorNoteRequest>,
    ) -> Result<Response<()>, Status> {
        let req = request.into_inner();
        info!("Appending moderator note for user: {}", req.user_id);
        
        let user_id = Uuid::parse_str(&req.user_id)
            .map_err(|e| Status::invalid_argument(format!("Invalid user_id: {}", e)))?;
        
        // Get or create analysis
        let mut analysis = match self.analysis_repo.get_analysis(user_id).await
            .map_err(|e| Status::internal(format!("Failed to get analysis: {}", e)))? {
            Some(existing) => existing,
            None => {
                // Create new analysis with just the note
                let new_analysis = UserAnalysisModel {
                    user_analysis_id: Uuid::new_v4(),
                    user_id,
                    spam_score: 0.0,
                    intelligibility_score: 0.0,
                    quality_score: 0.0,
                    horni_score: 0.0,
                    ai_notes: None,
                    moderator_notes: Some(req.note_text.clone()),
                    created_at: Utc::now(),
                    updated_at: Utc::now(),
                };
                
                self.analysis_repo.create_analysis(&new_analysis).await
                    .map_err(|e| Status::internal(format!("Failed to create analysis: {}", e)))?;
                
                return Ok(Response::new(()));
            }
        };
        
        // Append to existing notes
        let timestamp = Utc::now().format("%Y-%m-%d %H:%M:%S UTC").to_string();
        let new_note = format!("[{}] {}", timestamp, req.note_text);
        
        analysis.moderator_notes = match analysis.moderator_notes {
            Some(existing) => Some(format!("{}\n{}", existing, new_note)),
            None => Some(new_note),
        };
        
        analysis.updated_at = Utc::now();
        
        self.analysis_repo.update_analysis(&analysis).await
            .map_err(|e| Status::internal(format!("Failed to update analysis: {}", e)))?;
        
        Ok(Response::new(()))
    }
    
    type StreamUserUpdatesStream = tonic::codec::Streaming<UserUpdateEvent>;
    
    async fn stream_user_updates(
        &self,
        _request: Request<StreamUserUpdatesRequest>,
    ) -> Result<Response<Self::StreamUserUpdatesStream>, Status> {
        Err(Status::unimplemented("stream_user_updates not implemented"))
    }
}