use crate::GrpcClient;
use super::CommandError;
use maowbot_proto::maowbot::services::{
    GetUserRequest, SearchUsersRequest, SearchField, MergeUsersRequest,
    GetPlatformIdentitiesRequest, AddRoleToIdentityRequest, RemoveRoleFromIdentityRequest,
    GetUserAnalysisRequest, AppendModeratorNoteRequest, FindUserByNameRequest,
    MergeStrategy,
};
use maowbot_proto::maowbot::common::{User, PlatformIdentity, UserAnalysis};
use uuid::Uuid;

/// Result of user info query
pub struct UserInfoResult {
    pub user: User,
    pub identities: Vec<PlatformIdentity>,
    pub analysis: Option<UserAnalysis>,
}

/// Result of user search
pub struct UserSearchResult {
    pub users: Vec<User>,
}

/// Result of merge operation
pub struct MergeResult {
    pub merged_user: User,
    pub merged_count: usize,
}

/// Member command handlers
pub struct MemberCommands;

impl MemberCommands {
    /// Get comprehensive user information
    pub async fn get_user_info(
        client: &GrpcClient,
        identifier: &str,
    ) -> Result<UserInfoResult, CommandError> {
        let user = Self::resolve_user(client, identifier).await?;
        
        // Get platform identities
        let ident_request = GetPlatformIdentitiesRequest {
            user_id: user.user_id.clone(),
            platforms: vec![],
        };
        
        let mut user_client = client.user.clone();
        let identities = user_client
            .get_platform_identities(ident_request)
            .await
            .map_err(|e| CommandError::GrpcError(e.to_string()))?
            .into_inner()
            .identities;
            
        // Get user analysis
        let analysis_request = GetUserAnalysisRequest {
            user_id: user.user_id.clone(),
        };
        
        let analysis = match user_client.get_user_analysis(analysis_request).await {
            Ok(response) => response.into_inner().analysis,
            Err(_) => None,
        };
        
        Ok(UserInfoResult {
            user,
            identities,
            analysis,
        })
    }
    
    /// Search for users
    pub async fn search_users(
        client: &GrpcClient,
        query: &str,
    ) -> Result<UserSearchResult, CommandError> {
        let request = SearchUsersRequest {
            query: query.to_string(),
            search_fields: vec![SearchField::All as i32],
            page: None,
        };
        
        let mut user_client = client.user.clone();
        let response = user_client
            .search_users(request)
            .await
            .map_err(|e| CommandError::GrpcError(e.to_string()))?;
            
        let users = response
            .into_inner()
            .results
            .into_iter()
            .filter_map(|r| r.user)
            .collect();
            
        Ok(UserSearchResult { users })
    }
    
    /// Add moderator note
    pub async fn add_moderator_note(
        client: &GrpcClient,
        identifier: &str,
        note_text: &str,
    ) -> Result<(), CommandError> {
        let user = Self::resolve_user(client, identifier).await?;
        
        let request = AppendModeratorNoteRequest {
            user_id: user.user_id,
            note_text: note_text.to_string(),
            moderator_id: String::new(), // TODO: Get current moderator ID
        };
        
        let mut user_client = client.user.clone();
        user_client
            .append_moderator_note(request)
            .await
            .map_err(|e| CommandError::GrpcError(e.to_string()))?;
            
        Ok(())
    }
    
    /// Merge users
    pub async fn merge_users(
        client: &GrpcClient,
        user1_id: &str,
        user2_id: &str,
        new_global_name: Option<&str>,
    ) -> Result<MergeResult, CommandError> {
        // Parse UUIDs
        let uuid1 = Uuid::parse_str(user1_id)
            .map_err(|e| CommandError::InvalidInput(format!("Invalid UUID: {}", e)))?;
        let uuid2 = Uuid::parse_str(user2_id)
            .map_err(|e| CommandError::InvalidInput(format!("Invalid UUID: {}", e)))?;
            
        // Get both users to determine which is older
        let mut user_client = client.user.clone();
        
        let user1_request = GetUserRequest {
            user_id: uuid1.to_string(),
            include_identities: false,
            include_analysis: false,
        };
        
        let user2_request = GetUserRequest {
            user_id: uuid2.to_string(),
            include_identities: false,
            include_analysis: false,
        };
        
        let user1 = user_client
            .get_user(user1_request)
            .await
            .map_err(|e| CommandError::GrpcError(e.to_string()))?
            .into_inner()
            .user
            .ok_or_else(|| CommandError::NotFound("First user not found".to_string()))?;
            
        let user2 = user_client
            .get_user(user2_request)
            .await
            .map_err(|e| CommandError::GrpcError(e.to_string()))?
            .into_inner()
            .user
            .ok_or_else(|| CommandError::NotFound("Second user not found".to_string()))?;
            
        // Determine older user (target) and newer user (source)
        // Compare timestamps by converting to seconds
        let user1_time = user1.created_at.as_ref()
            .map(|t| t.seconds)
            .unwrap_or(0);
        let user2_time = user2.created_at.as_ref()
            .map(|t| t.seconds)
            .unwrap_or(0);
            
        let (source, target) = if user1_time <= user2_time {
            (user2.user_id, user1.user_id)
        } else {
            (user1.user_id, user2.user_id)
        };
        
        let request = MergeUsersRequest {
            source_user_id: source.clone(),
            target_user_id: target.clone(),
            new_global_name: new_global_name.unwrap_or_default().to_string(),
            strategy: MergeStrategy::KeepTarget as i32,
        };
        
        let response = user_client
            .merge_users(request)
            .await
            .map_err(|e| CommandError::GrpcError(e.to_string()))?
            .into_inner();
            
        Ok(MergeResult {
            merged_user: response.merged_user.unwrap_or_default(),
            merged_count: 1,
        })
    }
    
    /// Merge duplicate users by username
    pub async fn merge_duplicates(
        client: &GrpcClient,
        username: &str,
        new_global_name: Option<&str>,
    ) -> Result<MergeResult, CommandError> {
        // Find all users with this username
        let all_matches = Self::search_users(client, username).await?.users;
        
        let matching: Vec<User> = all_matches
            .into_iter()
            .filter(|u| {
                u.global_username.to_lowercase() == username.to_lowercase()
            })
            .collect();
            
        if matching.is_empty() {
            return Err(CommandError::NotFound(format!("No users found with username '{}'", username)));
        }
        
        if matching.len() == 1 {
            return Err(CommandError::InvalidInput("Only one user found, no duplicates to merge".to_string()));
        }
        
        // Sort by creation date
        let mut sorted = matching;
        sorted.sort_by_key(|u| u.created_at.as_ref().map(|t| t.seconds).unwrap_or(0));
        let oldest = &sorted[0];
        let others = &sorted[1..];
        
        let mut user_client = client.user.clone();
        
        // Merge each duplicate into the oldest
        for dup in others {
            let request = MergeUsersRequest {
                source_user_id: dup.user_id.clone(),
                target_user_id: oldest.user_id.clone(),
                new_global_name: String::new(),
                strategy: MergeStrategy::KeepTarget as i32,
            };
            
            user_client
                .merge_users(request)
                .await
                .map_err(|e| CommandError::GrpcError(format!(
                    "Failed to merge {} into {}: {}",
                    dup.user_id, oldest.user_id, e
                )))?;
        }
        
        // If new name provided, update the merged user
        let final_user = if let Some(new_name) = new_global_name {
            let request = MergeUsersRequest {
                source_user_id: oldest.user_id.clone(),
                target_user_id: oldest.user_id.clone(),
                new_global_name: new_name.to_string(),
                strategy: MergeStrategy::KeepTarget as i32,
            };
            
            user_client
                .merge_users(request)
                .await
                .map_err(|e| CommandError::GrpcError(e.to_string()))?
                .into_inner()
                .merged_user
                .unwrap_or_else(|| oldest.clone())
        } else {
            oldest.clone()
        };
        
        Ok(MergeResult {
            merged_user: final_user,
            merged_count: others.len(),
        })
    }
    
    /// Add role to user identity
    pub async fn add_role(
        client: &GrpcClient,
        identifier: &str,
        platform: &str,
        role: &str,
    ) -> Result<(), CommandError> {
        let user = Self::resolve_user(client, identifier).await?;
        
        let request = AddRoleToIdentityRequest {
            user_id: user.user_id,
            platform: platform.to_string(),
            role: role.to_string(),
        };
        
        let mut user_client = client.user.clone();
        user_client
            .add_role_to_identity(request)
            .await
            .map_err(|e| CommandError::GrpcError(e.to_string()))?;
            
        Ok(())
    }
    
    /// Remove role from user identity
    pub async fn remove_role(
        client: &GrpcClient,
        identifier: &str,
        platform: &str,
        role: &str,
    ) -> Result<(), CommandError> {
        let user = Self::resolve_user(client, identifier).await?;
        
        let request = RemoveRoleFromIdentityRequest {
            user_id: user.user_id,
            platform: platform.to_string(),
            role: role.to_string(),
        };
        
        let mut user_client = client.user.clone();
        user_client
            .remove_role_from_identity(request)
            .await
            .map_err(|e| CommandError::GrpcError(e.to_string()))?;
            
        Ok(())
    }
    
    /// Resolve user by name or UUID
    async fn resolve_user(client: &GrpcClient, identifier: &str) -> Result<User, CommandError> {
        // Try to parse as UUID first
        if let Ok(uuid) = Uuid::parse_str(identifier) {
            let request = GetUserRequest {
                user_id: uuid.to_string(),
                include_identities: false,
                include_analysis: false,
            };
            
            let mut user_client = client.user.clone();
            let response = user_client
                .get_user(request)
                .await
                .map_err(|e| CommandError::GrpcError(e.to_string()))?;
                
            response
                .into_inner()
                .user
                .ok_or_else(|| CommandError::NotFound("User not found".to_string()))
        } else {
            // Look up by name
            let request = FindUserByNameRequest {
                name: identifier.to_string(),
                exact_match: true,
            };
            
            let mut user_client = client.user.clone();
            let response = user_client
                .find_user_by_name(request)
                .await
                .map_err(|e| CommandError::GrpcError(e.to_string()))?;
                
            let users = response.into_inner().users;
            users
                .into_iter()
                .next()
                .ok_or_else(|| CommandError::NotFound(format!("No user found with name '{}'", identifier)))
        }
    }
}