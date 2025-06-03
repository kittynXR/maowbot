use tonic::{Request, Response, Status};
use maowbot_proto::maowbot::services::{vr_chat_service_server::VrChatService, *};
use maowbot_proto::maowbot::common;
use maowbot_core::plugins::manager::PluginManager;
use maowbot_common::traits::api::VrchatApi;
use std::sync::Arc;
use chrono::Utc;
use tracing::{info, error, debug};
use prost_types;
use uuid::Uuid;

pub struct VRChatServiceImpl {
    plugin_manager: Arc<PluginManager>,
}

impl VRChatServiceImpl {
    pub fn new(plugin_manager: Arc<PluginManager>) -> Self {
        Self {
            plugin_manager,
        }
    }
}

#[tonic::async_trait]
impl VrChatService for VRChatServiceImpl {
    async fn get_current_user(&self, request: Request<GetCurrentUserRequest>) -> Result<Response<GetCurrentUserResponse>, Status> {
        let req = request.into_inner();
        debug!("Getting current VRChat user for account: {}", req.account_name);
        
        // TODO: Implement user retrieval through VRChat API
        // For now, return mock data
        let user = VrChatUser {
            user_id: "usr_12345678-1234-1234-1234-123456789012".to_string(),
            display_name: req.account_name.clone(),
            username: req.account_name,
            status: "active".to_string(),
            status_description: "Playing VRChat".to_string(),
            bio: "Mock VRChat user".to_string(),
            current_avatar_id: "avtr_12345678-1234-1234-1234-123456789012".to_string(),
            current_avatar_thumbnail: String::new(),
            home_location: "wrld_home".to_string(),
            world_id: "wrld_public".to_string(),
            instance_id: "12345".to_string(),
            tags: vec![],
            online_status: OnlineStatus::Active as i32,
            last_login: None,
        };
        
        Ok(Response::new(GetCurrentUserResponse {
            user: Some(user),
        }))
    }
    async fn update_user_status(&self, request: Request<UpdateUserStatusRequest>) -> Result<Response<()>, Status> {
        let req = request.into_inner();
        info!("Updating VRChat user status for account: {} to {}", req.account_name, req.status);
        
        // TODO: Implement status update through VRChat API
        Err(Status::unimplemented("User status update not yet implemented"))
    }
    async fn get_current_world(&self, request: Request<GetCurrentWorldRequest>) -> Result<Response<GetCurrentWorldResponse>, Status> {
        let req = request.into_inner();
        debug!("Getting current VRChat world for account: {}", req.account_name);
        
        let pm = &self.plugin_manager;
        
        let world_basic = pm.vrchat_get_current_world(&req.account_name).await
            .map_err(|e| Status::internal(format!("Failed to get current world: {}", e)))?;
        
        // Convert basic world info to full proto format
        let world = VrChatWorld {
            world_id: String::new(), // Basic doesn't include world_id
            name: world_basic.name,
            description: world_basic.description,
            image_url: String::new(),
            thumbnail_url: String::new(),
            author_id: String::new(),
            author_name: world_basic.author_name,
            capacity: world_basic.capacity as i32,
            tags: vec![],
            release_status: match world_basic.release_status.as_str() {
                "public" => ReleaseStatus::Public as i32,
                "private" => ReleaseStatus::Private as i32,
                "hidden" => ReleaseStatus::Hidden as i32,
                _ => ReleaseStatus::Unknown as i32,
            },
            occupants: 0,
            
            favorites: 0,
            
            
            created_at: world_basic.created_at.parse::<chrono::DateTime<chrono::Utc>>()
                .ok()
                .map(|dt| prost_types::Timestamp {
                    seconds: dt.timestamp(),
                    nanos: dt.timestamp_subsec_nanos() as i32,
                }),
            updated_at: world_basic.updated_at.parse::<chrono::DateTime<chrono::Utc>>()
                .ok()
                .map(|dt| prost_types::Timestamp {
                    seconds: dt.timestamp(),
                    nanos: dt.timestamp_subsec_nanos() as i32,
                }),
        };
        
        Ok(Response::new(GetCurrentWorldResponse {
            world: Some(world),
            instance: None, // TODO: Populate instance info
        }))
    }
    async fn get_world(&self, request: Request<GetWorldRequest>) -> Result<Response<GetWorldResponse>, Status> {
        let req = request.into_inner();
        debug!("Getting VRChat world: {}", req.world_id);
        
        // TODO: Implement world retrieval through VRChat API
        // For now, return mock data
        let world = VrChatWorld {
            world_id: req.world_id.clone(),
            name: "Mock World".to_string(),
            description: "A mock VRChat world".to_string(),
            image_url: "https://example.com/world.jpg".to_string(),
            thumbnail_url: "https://example.com/world_thumb.jpg".to_string(),
            author_id: "usr_author".to_string(),
            author_name: "MockAuthor".to_string(),
            capacity: 20,
            tags: vec!["social".to_string()],
            release_status: ReleaseStatus::Public as i32,
            occupants: 5,
            
            favorites: 100,
            
            
            created_at: Some(prost_types::Timestamp {
                seconds: Utc::now().timestamp() - 86400 * 30,
                nanos: 0,
            }),
            updated_at: Some(prost_types::Timestamp {
                seconds: Utc::now().timestamp() - 86400,
                nanos: 0,
            }),
        };
        
        Ok(Response::new(GetWorldResponse {
            world: Some(world),
        }))
    }
    async fn get_current_instance(&self, request: Request<GetCurrentInstanceRequest>) -> Result<Response<GetCurrentInstanceResponse>, Status> {
        let req = request.into_inner();
        debug!("Getting current VRChat instance for account: {}", req.account_name);
        
        let pm = &self.plugin_manager;
        
        let instance_basic = pm.vrchat_get_current_instance(&req.account_name).await
            .map_err(|e| Status::internal(format!("Failed to get current instance: {}", e)))?;
        
        // Convert basic instance info to full proto format
        let instance = VrChatInstance {
            instance_id: instance_basic.instance_id.unwrap_or_default(),
            world_id: instance_basic.world_id.unwrap_or_default(),
            r#type: InstanceType::Public as i32, // Default to public since we don't have type info
            owner_id: String::new(),
            user_count: 0,
            capacity: 0,
            user_ids: vec![],
        };
        
        Ok(Response::new(GetCurrentInstanceResponse {
            instance: Some(instance),
            users: vec![], // TODO: Populate users in instance
        }))
    }
    async fn join_world(&self, request: Request<JoinWorldRequest>) -> Result<Response<JoinWorldResponse>, Status> {
        let req = request.into_inner();
        info!("Joining VRChat world {} with instance {}", req.world_id, req.instance_id);
        
        // TODO: Implement world joining through VRChat API
        Err(Status::unimplemented("World joining not yet implemented"))
    }
    async fn invite_to_world(&self, request: Request<InviteToWorldRequest>) -> Result<Response<()>, Status> {
        let req = request.into_inner();
        info!("Inviting user {} to world", req.user_id);
        
        // TODO: Implement world invitation through VRChat API
        Err(Status::unimplemented("World invitation not yet implemented"))
    }
    async fn get_current_avatar(&self, request: Request<GetCurrentAvatarRequest>) -> Result<Response<GetCurrentAvatarResponse>, Status> {
        let req = request.into_inner();
        debug!("Getting current VRChat avatar for account: {}", req.account_name);
        
        let pm = &self.plugin_manager;
        
        let avatar_basic = pm.vrchat_get_current_avatar(&req.account_name).await
            .map_err(|e| Status::internal(format!("Failed to get current avatar: {}", e)))?;
        
        // Convert basic avatar info to full proto format
        let avatar = VrChatAvatar {
            avatar_id: avatar_basic.avatar_id,
            name: avatar_basic.avatar_name,
            description: String::new(),
            author_id: String::new(),
            author_name: String::new(),
            tags: vec![],
            
            image_url: String::new(),
            thumbnail_url: String::new(),
            release_status: ReleaseStatus::Public as i32,
            version: 1,
            parameters: vec![],
            created_at: None,
            updated_at: None,
        };
        
        Ok(Response::new(GetCurrentAvatarResponse {
            avatar: Some(avatar),
        }))
    }
    async fn get_avatar(&self, request: Request<GetAvatarRequest>) -> Result<Response<GetAvatarResponse>, Status> {
        let req = request.into_inner();
        debug!("Getting VRChat avatar: {}", req.avatar_id);
        
        // TODO: Implement avatar retrieval through VRChat API
        // For now, return mock data
        let avatar = VrChatAvatar {
            avatar_id: req.avatar_id.clone(),
            name: "Mock Avatar".to_string(),
            description: "A mock VRChat avatar".to_string(),
            author_id: "usr_author".to_string(),
            author_name: "MockAuthor".to_string(),
            tags: vec!["avatar".to_string()],
            image_url: "https://example.com/avatar.jpg".to_string(),
            thumbnail_url: "https://example.com/avatar_thumb.jpg".to_string(),
            release_status: ReleaseStatus::Public as i32,
            version: 1,
            parameters: vec![],
            created_at: Some(prost_types::Timestamp {
                seconds: Utc::now().timestamp() - 86400 * 30,
                nanos: 0,
            }),
            updated_at: Some(prost_types::Timestamp {
                seconds: Utc::now().timestamp() - 86400,
                nanos: 0,
            }),
        };
        
        Ok(Response::new(GetAvatarResponse {
            avatar: Some(avatar),
        }))
    }
    async fn change_avatar(&self, request: Request<ChangeAvatarRequest>) -> Result<Response<ChangeAvatarResponse>, Status> {
        let req = request.into_inner();
        info!("Changing VRChat avatar to: {} for account: {}", req.avatar_id, req.account_name);
        
        let pm = &self.plugin_manager;
        
        pm.vrchat_change_avatar(&req.account_name, &req.avatar_id).await
            .map_err(|e| Status::internal(format!("Failed to change avatar: {}", e)))?;
        
        // Get the new current avatar to return
        let avatar_basic = pm.vrchat_get_current_avatar(&req.account_name).await
            .map_err(|e| Status::internal(format!("Failed to get new avatar: {}", e)))?;
        
        // Convert to proto format
        let avatar = VrChatAvatar {
            avatar_id: avatar_basic.avatar_id,
            name: avatar_basic.avatar_name,
            description: String::new(),
            author_id: String::new(),
            author_name: String::new(),
            tags: vec![],
            
            image_url: String::new(),
            thumbnail_url: String::new(),
            release_status: ReleaseStatus::Public as i32,
            version: 1,
            parameters: vec![],
            created_at: None,
            updated_at: None,
        };
        
        Ok(Response::new(ChangeAvatarResponse {
            avatar: Some(avatar),
            success: true,
            error_message: String::new(),
        }))
    }
    async fn list_avatars(&self, request: Request<ListAvatarsRequest>) -> Result<Response<ListAvatarsResponse>, Status> {
        let req = request.into_inner();
        debug!("Listing VRChat avatars for account: {}", req.account_name);
        
        // TODO: Implement avatar listing through VRChat API
        // For now, return empty list
        Ok(Response::new(ListAvatarsResponse {
            avatars: vec![],
            page: None,
        }))
    }
    async fn get_avatar_parameters(&self, request: Request<GetAvatarParametersRequest>) -> Result<Response<GetAvatarParametersResponse>, Status> {
        let req = request.into_inner();
        debug!("Getting avatar parameters for account: {}", req.account_name);
        
        // TODO: Implement avatar parameter retrieval through OSC
        // For now, return empty parameters
        Ok(Response::new(GetAvatarParametersResponse {
            parameters: vec![],
        }))
    }
    async fn list_friends(&self, request: Request<ListFriendsRequest>) -> Result<Response<ListFriendsResponse>, Status> {
        let req = request.into_inner();
        debug!("Listing VRChat friends for account: {}", req.account_name);
        
        // TODO: Implement friend listing through VRChat API
        // For now, return empty list
        Ok(Response::new(ListFriendsResponse {
            friends: vec![],
            page: None,
        }))
    }
    async fn get_friend(&self, request: Request<GetFriendRequest>) -> Result<Response<GetFriendResponse>, Status> {
        let req = request.into_inner();
        debug!("Getting VRChat friend: {}", req.user_id);
        
        // TODO: Implement friend retrieval through VRChat API
        // For now, return mock data
        let friend = VrChatFriend {
            user_id: req.user_id.clone(),
            display_name: "Mock Friend".to_string(),
            status: "active".to_string(),
            status_description: "Playing VRChat".to_string(),
            location: "wrld_public".to_string(),
            current_avatar_thumbnail: String::new(),
            online_status: OnlineStatus::Active as i32,
            last_login: Some(prost_types::Timestamp {
                seconds: Utc::now().timestamp() - 86400 * 30,
                nanos: 0,
            }),
        };
        
        Ok(Response::new(GetFriendResponse {
            friend: Some(friend),
        }))
    }
    async fn send_friend_request(&self, request: Request<SendFriendRequestRequest>) -> Result<Response<()>, Status> {
        let req = request.into_inner();
        info!("Sending friend request to user: {}", req.user_id);
        
        // TODO: Implement friend request through VRChat API
        Err(Status::unimplemented("Friend request sending not yet implemented"))
    }
    async fn accept_friend_request(&self, request: Request<AcceptFriendRequestRequest>) -> Result<Response<()>, Status> {
        let req = request.into_inner();
        info!("Accepting friend request notification: {}", req.notification_id);
        
        // TODO: Implement friend request acceptance through VRChat API
        Err(Status::unimplemented("Friend request acceptance not yet implemented"))
    }
    async fn list_notifications(&self, request: Request<ListNotificationsRequest>) -> Result<Response<ListNotificationsResponse>, Status> {
        let req = request.into_inner();
        debug!("Listing VRChat notifications for account: {}", req.account_name);
        
        // TODO: Implement notification listing through VRChat API
        // For now, return empty list
        Ok(Response::new(ListNotificationsResponse {
            notifications: vec![],
            page: None, // TODO: Add pagination support
        }))
    }
    async fn send_notification(&self, request: Request<SendNotificationRequest>) -> Result<Response<()>, Status> {
        let req = request.into_inner();
        info!("Sending VRChat notification to user: {}", req.user_id);
        
        // TODO: Implement notification sending through VRChat API
        Err(Status::unimplemented("Notification sending not yet implemented"))
    }
    async fn clear_notification(&self, request: Request<ClearNotificationRequest>) -> Result<Response<()>, Status> {
        let req = request.into_inner();
        info!("Clearing VRChat notification: {}", req.notification_id);
        
        // TODO: Implement notification clearing through VRChat API
        Err(Status::unimplemented("Notification clearing not yet implemented"))
    }
    type StreamVRChatEventsStream = tonic::codec::Streaming<VrChatEvent>;
    async fn stream_vr_chat_events(&self, _: Request<StreamVrChatEventsRequest>) -> Result<Response<Self::StreamVRChatEventsStream>, Status> {
        Err(Status::unimplemented("Not implemented"))
    }
}