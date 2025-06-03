use crate::GrpcClient;
use super::{CommandResult, CommandError};
use maowbot_proto::maowbot::services::{
    CreateUserRequest, DeleteUserRequest, UpdateUserRequest, GetUserRequest,
    SearchUsersRequest, ListUsersRequest, GetPlatformIdentitiesRequest,
    SearchField, ListUsersFilter,
};
use maowbot_proto::maowbot::common::{PageRequest, User as ProtoUser, PlatformIdentity};
use uuid::Uuid;

/// Result of creating a user
pub struct CreateUserResult {
    pub user: ProtoUser,
}

/// Result of deleting a user
pub struct DeleteUserResult {
    pub user_id: String,
    pub was_hard_delete: bool,
}

/// Result of updating a user
pub struct UpdateUserResult {
    pub user: ProtoUser,
    pub fields_updated: Vec<String>,
}

/// Result of user info query
pub struct UserInfoResult {
    pub user: ProtoUser,
    pub identities: Vec<PlatformIdentity>,
}

/// Result of user search
pub struct SearchUsersResult {
    pub users: Vec<ProtoUser>,
    pub total_count: i32,
}

/// Result of listing users
pub struct ListUsersResult {
    pub users: Vec<ProtoUser>,
    pub total_count: i32,
    pub has_more: bool,
    pub next_page_token: String,
}

/// User command handlers
pub struct UserCommands;

impl UserCommands {
    /// Create a new user
    pub async fn create_user(
        client: &GrpcClient,
        username: &str,
        is_active: bool,
    ) -> Result<CommandResult<CreateUserResult>, CommandError> {
        let request = CreateUserRequest {
            user_id: String::new(), // Let server generate
            display_name: username.to_string(),
            is_active,
        };
        
        match client.user.clone().create_user(request).await {
            Ok(response) => {
                let user = response.into_inner().user
                    .ok_or_else(|| CommandError::DataError("No user in response".to_string()))?;
                Ok(CommandResult::new(CreateUserResult { user }))
            }
            Err(e) => Err(CommandError::GrpcError(e.to_string())),
        }
    }
    
    /// Delete a user by ID or username
    pub async fn delete_user(
        client: &GrpcClient,
        username_or_id: &str,
        hard_delete: bool,
    ) -> Result<CommandResult<DeleteUserResult>, CommandError> {
        // First, resolve the user ID
        let user_id = if let Ok(uuid) = Uuid::parse_str(username_or_id) {
            uuid.to_string()
        } else {
            // Look up by username
            match Self::find_user_by_name(client, username_or_id).await? {
                Some(user) => user.user_id,
                None => return Err(CommandError::NotFound(format!("User not found: {}", username_or_id))),
            }
        };
        
        let request = DeleteUserRequest {
            user_id: user_id.clone(),
            hard_delete,
        };
        
        match client.user.clone().delete_user(request).await {
            Ok(_) => Ok(CommandResult::new(DeleteUserResult {
                user_id,
                was_hard_delete: hard_delete,
            })),
            Err(e) => Err(CommandError::GrpcError(e.to_string())),
        }
    }
    
    /// Update a user
    pub async fn update_user(
        client: &GrpcClient,
        user_id: &str,
        updates: UserUpdates,
    ) -> Result<CommandResult<UpdateUserResult>, CommandError> {
        // Get the current user first
        let user = Self::get_user_by_id_or_name(client, user_id).await?
            .ok_or_else(|| CommandError::NotFound(format!("User not found: {}", user_id)))?;
        
        let mut updated_user = user.clone();
        let mut fields_updated = Vec::new();
        
        if let Some(is_active) = updates.is_active {
            updated_user.is_active = is_active;
            fields_updated.push("is_active".to_string());
        }
        
        if let Some(username) = updates.username {
            updated_user.global_username = username;
            fields_updated.push("global_username".to_string());
        }
        
        let request = UpdateUserRequest {
            user_id: user.user_id,
            user: Some(updated_user),
            update_mask: None, // TODO: When prost_types is available
        };
        
        match client.user.clone().update_user(request).await {
            Ok(response) => {
                let user = response.into_inner().user
                    .ok_or_else(|| CommandError::DataError("No user in response".to_string()))?;
                Ok(CommandResult::new(UpdateUserResult {
                    user,
                    fields_updated,
                }))
            }
            Err(e) => Err(CommandError::GrpcError(e.to_string())),
        }
    }
    
    /// Get detailed user information
    pub async fn get_user_info(
        client: &GrpcClient,
        username_or_id: &str,
    ) -> Result<CommandResult<UserInfoResult>, CommandError> {
        let user = Self::get_user_by_id_or_name(client, username_or_id).await?
            .ok_or_else(|| CommandError::NotFound(format!("User not found: {}", username_or_id)))?;
        
        // Get platform identities
        let identities_request = GetPlatformIdentitiesRequest {
            user_id: user.user_id.clone(),
            platforms: vec![], // All platforms
        };
        
        let identities = match client.user.clone().get_platform_identities(identities_request).await {
            Ok(response) => response.into_inner().identities,
            Err(_) => vec![], // Ignore errors, just return empty
        };
        
        Ok(CommandResult::new(UserInfoResult { user, identities }))
    }
    
    /// Search for users
    pub async fn search_users(
        client: &GrpcClient,
        query: &str,
        limit: i32,
    ) -> Result<CommandResult<SearchUsersResult>, CommandError> {
        let request = SearchUsersRequest {
            query: query.to_string(),
            search_fields: vec![SearchField::Username as i32],
            page: Some(PageRequest {
                page_size: limit,
                page_token: String::new(),
            }),
        };
        
        match client.user.clone().search_users(request).await {
            Ok(response) => {
                let resp = response.into_inner();
                let page_info = resp.page.unwrap_or_default();
                let users = resp.results.into_iter()
                    .filter_map(|r| r.user)
                    .collect();
                    
                Ok(CommandResult::new(SearchUsersResult {
                    users,
                    total_count: page_info.total_count,
                }))
            }
            Err(e) => Err(CommandError::GrpcError(e.to_string())),
        }
    }
    
    /// List users with pagination
    pub async fn list_users(
        client: &GrpcClient,
        page_size: i32,
        page_token: Option<String>,
        active_only: bool,
    ) -> Result<CommandResult<ListUsersResult>, CommandError> {
        let request = ListUsersRequest {
            page: Some(PageRequest {
                page_size,
                page_token: page_token.unwrap_or_default(),
            }),
            filter: Some(ListUsersFilter {
                active_only,
                platforms: vec![],
                roles: vec![],
            }),
            order_by: "created_at".to_string(),
            descending: false,
        };
        
        match client.user.clone().list_users(request).await {
            Ok(response) => {
                let resp = response.into_inner();
                let page_info = resp.page.unwrap_or_default();
                
                Ok(CommandResult::new(ListUsersResult {
                    users: resp.users,
                    total_count: page_info.total_count,
                    has_more: !page_info.next_page_token.is_empty(),
                    next_page_token: page_info.next_page_token,
                }))
            }
            Err(e) => Err(CommandError::GrpcError(e.to_string())),
        }
    }
    
    // Helper methods
    async fn find_user_by_name(client: &GrpcClient, username: &str) -> Result<Option<ProtoUser>, CommandError> {
        let result = Self::search_users(client, username, 1).await?;
        Ok(result.data.users.into_iter().next())
    }
    
    async fn get_user_by_id_or_name(client: &GrpcClient, username_or_id: &str) -> Result<Option<ProtoUser>, CommandError> {
        if let Ok(uuid) = Uuid::parse_str(username_or_id) {
            let request = GetUserRequest {
                user_id: uuid.to_string(),
                include_identities: false,
                include_analysis: false,
            };
            
            match client.user.clone().get_user(request).await {
                Ok(response) => Ok(response.into_inner().user),
                Err(_) => Ok(None),
            }
        } else {
            Self::find_user_by_name(client, username_or_id).await
        }
    }
}

/// User update options
pub struct UserUpdates {
    pub is_active: Option<bool>,
    pub username: Option<String>,
}