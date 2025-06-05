use tonic::{Request, Response, Status};
use maowbot_proto::maowbot::services::{discord_service_server::DiscordService, *};
use maowbot_proto::maowbot::common;
use maowbot_core::plugins::manager::PluginManager;
use maowbot_core::repositories::postgres::discord::PostgresDiscordRepository;
use maowbot_common::traits::api::DiscordApi;
use std::sync::Arc;
use std::collections::HashMap;
use chrono::Utc;
use tracing::{info, error, debug};
use prost_types;
use uuid::Uuid;

pub struct DiscordServiceImpl {
    plugin_manager: Arc<PluginManager>,
    discord_repo: Arc<PostgresDiscordRepository>,
}

impl DiscordServiceImpl {
    pub fn new(plugin_manager: Arc<PluginManager>, discord_repo: Arc<PostgresDiscordRepository>) -> Self {
        Self {
            plugin_manager,
            discord_repo,
        }
    }
}

#[tonic::async_trait]
impl DiscordService for DiscordServiceImpl {
    async fn list_guilds(&self, request: Request<ListGuildsRequest>) -> Result<Response<ListGuildsResponse>, Status> {
        let req = request.into_inner();
        debug!("Listing Discord guilds for account: {}", req.account_name);
        
        // Use the high-level API to list guilds
        let guild_records = self.plugin_manager.list_discord_guilds(&req.account_name).await
            .map_err(|e| Status::internal(format!("Failed to list guilds: {}", e)))?;
        
        // Convert to proto format
        let guilds: Vec<Guild> = guild_records.into_iter()
            .map(|record| Guild {
                guild_id: record.guild_id,
                name: record.guild_name,
                icon_url: String::new(), // TODO: Construct icon URL
                is_owner: false,
                features: vec![],
                member_count: 0,
            })
            .collect();
        
        Ok(Response::new(ListGuildsResponse {
            guilds,
        }))
    }
    async fn get_guild(&self, request: Request<GetGuildRequest>) -> Result<Response<GetGuildResponse>, Status> {
        let req = request.into_inner();
        debug!("Getting Discord guild: {}", req.guild_id);
        
        // TODO: Implement guild details retrieval through Discord API
        // For now, return mock data
        let guild = Guild {
            guild_id: req.guild_id.clone(),
            name: "Mock Guild".to_string(),
            icon_url: String::new(),
            is_owner: false,
            features: vec![],
            member_count: 100,
        };
        
        Ok(Response::new(GetGuildResponse {
            guild: Some(guild),
            settings: Some(GuildSettings {
                prefix: "!".to_string(),
                enabled_features: vec![],
                custom_settings: HashMap::new(),
            }),
        }))
    }
    async fn list_channels(&self, request: Request<ListChannelsRequest>) -> Result<Response<ListChannelsResponse>, Status> {
        let req = request.into_inner();
        debug!("Listing Discord channels for guild: {}", req.guild_id);
        
        // Use the high-level API to list channels
        let channel_records = self.plugin_manager.list_discord_channels(&req.account_name, &req.guild_id).await
            .map_err(|e| Status::internal(format!("Failed to list channels: {}", e)))?;
        
        // Convert to proto format
        let channels: Vec<Channel> = channel_records.into_iter()
            .map(|record| Channel {
                channel_id: record.channel_id,
                guild_id: record.guild_id,
                name: record.channel_name,
                r#type: ChannelType::Text as i32, // Default to text channel since we don't store type
                position: 0,
                parent_id: String::new(),
                topic: String::new(),
                is_nsfw: false,
                overwrites: vec![],
            })
            .collect();
        
        Ok(Response::new(ListChannelsResponse {
            channels,
        }))
    }
    async fn get_channel(&self, request: Request<GetChannelRequest>) -> Result<Response<GetChannelResponse>, Status> {
        let req = request.into_inner();
        debug!("Getting Discord channel: {}", req.channel_id);
        
        // TODO: Implement channel details retrieval through Discord API
        // For now, return mock data
        let channel = Channel {
            channel_id: req.channel_id.clone(),
            guild_id: String::new(), // TODO: Get guild_id
            name: "general".to_string(),
            r#type: ChannelType::Text as i32,
            position: 0,
            parent_id: String::new(),
            topic: "General discussion".to_string(),
            is_nsfw: false,
            overwrites: vec![],
        };
        
        Ok(Response::new(GetChannelResponse {
            channel: Some(channel),
        }))
    }
    async fn send_message(&self, request: Request<SendDiscordMessageRequest>) -> Result<Response<SendDiscordMessageResponse>, Status> {
        let req = request.into_inner();
        debug!("Sending Discord message to channel: {}", req.channel_id);
        
        let pm = &self.plugin_manager;
        
        // Send the message
        // TODO: Get guild_id from channel lookup
        let guild_id = String::new();
        pm.send_discord_message(&req.account_name, &guild_id, &req.channel_id, &req.content).await
            .map_err(|e| Status::internal(format!("Failed to send message: {}", e)))?;
        
        // Generate mock response data
        let message_id = Uuid::new_v4().to_string();
        let sent_at = Utc::now();
        
        Ok(Response::new(SendDiscordMessageResponse {
            message_id,
            timestamp: Some(prost_types::Timestamp {
                seconds: sent_at.timestamp(),
                nanos: sent_at.timestamp_subsec_nanos() as i32,
            }),
        }))
    }
    async fn edit_message(&self, request: Request<EditMessageRequest>) -> Result<Response<()>, Status> {
        let req = request.into_inner();
        info!("Editing Discord message {} in channel {}", req.message_id, req.channel_id);
        
        // TODO: Implement message editing through Discord API
        Err(Status::unimplemented("Message editing not yet implemented"))
    }
    async fn delete_message(&self, request: Request<DeleteDiscordMessageRequest>) -> Result<Response<()>, Status> {
        let req = request.into_inner();
        info!("Deleting Discord message {} in channel {}", req.message_id, req.channel_id);
        
        // TODO: Implement message deletion through Discord API
        Err(Status::unimplemented("Message deletion not yet implemented"))
    }
    async fn send_embed(&self, request: Request<SendEmbedRequest>) -> Result<Response<SendEmbedResponse>, Status> {
        let req = request.into_inner();
        debug!("Sending Discord embed to channel: {}", req.channel_id);
        
        let pm = &self.plugin_manager;
        
        if let Some(embed_proto) = req.embed {
            // Convert proto embed to Discord embed
            let embed = maowbot_common::models::discord::DiscordEmbed {
                title: if embed_proto.title.is_empty() { None } else { Some(embed_proto.title) },
                description: if embed_proto.description.is_empty() { None } else { Some(embed_proto.description) },
                url: if embed_proto.url.is_empty() { None } else { Some(embed_proto.url) },
                color: Some(maowbot_common::models::discord::DiscordColor(embed_proto.color as u32)),
                timestamp: embed_proto.timestamp.and_then(|ts| chrono::DateTime::from_timestamp(ts.seconds, 0)),
                footer: embed_proto.footer.map(|f| maowbot_common::models::discord::DiscordEmbedFooter {
                    text: f.text,
                    icon_url: if f.icon_url.is_empty() { None } else { Some(f.icon_url) },
                }),
                author: embed_proto.author.map(|a| maowbot_common::models::discord::DiscordEmbedAuthor {
                    name: a.name,
                    url: if a.url.is_empty() { None } else { Some(a.url) },
                    icon_url: if a.icon_url.is_empty() { None } else { Some(a.icon_url) },
                }),
                fields: embed_proto.fields.into_iter()
                    .map(|f| maowbot_common::models::discord::DiscordEmbedField {
                        name: f.name,
                        value: f.value,
                        inline: f.inline,
                    })
                    .collect(),
                thumbnail: embed_proto.thumbnail.map(|t| maowbot_common::models::discord::DiscordEmbedThumbnail {
                    url: t.url,
                }),
                image: embed_proto.image.map(|i| maowbot_common::models::discord::DiscordEmbedImage {
                    url: i.url,
                }),
            };
            
            // Send the embed
            // TODO: Get guild_id from channel lookup
            let guild_id = String::new();
            let content = None; // SendEmbedRequest doesn't have content field
            pm.send_discord_embed(&req.account_name, &guild_id, &req.channel_id, &embed, content).await
                .map_err(|e| Status::internal(format!("Failed to send embed: {}", e)))?;
        }
        
        // Generate mock response data
        let message_id = Uuid::new_v4().to_string();
        let sent_at = Utc::now();
        
        Ok(Response::new(SendEmbedResponse {
            message_id,
        }))
    }
    async fn list_roles(&self, request: Request<ListRolesRequest>) -> Result<Response<ListRolesResponse>, Status> {
        let req = request.into_inner();
        debug!("Listing Discord roles for guild: {}", req.guild_id);
        
        // Get role list from Discord API
        let role_tuples = self.plugin_manager.list_discord_roles(&req.account_name, &req.guild_id).await
            .map_err(|e| Status::internal(format!("Failed to list roles: {}", e)))?;
        
        // Convert to proto format
        let roles: Vec<Role> = role_tuples.into_iter()
            .map(|(role_id, role_name)| Role {
                role_id: role_id,
                guild_id: req.guild_id.clone(),
                name: role_name,
                color: 0,
                hoist: false,
                position: 0,
                permissions: 0,
                managed: false,
                mentionable: false,
            })
            .collect();
        
        Ok(Response::new(ListRolesResponse {
            roles,
        }))
    }
    async fn add_role_to_user(&self, request: Request<AddRoleToUserRequest>) -> Result<Response<()>, Status> {
        let req = request.into_inner();
        info!("Adding role {} to user {} in guild {}", req.role_id, req.user_id, req.guild_id);
        
        let pm = &self.plugin_manager;
        
        pm.add_role_to_discord_user(&req.account_name, &req.guild_id, &req.user_id, &req.role_id).await
            .map_err(|e| Status::internal(format!("Failed to add role: {}", e)))?;
        
        Ok(Response::new(()))
    }
    async fn remove_role_from_user(&self, request: Request<RemoveRoleFromUserRequest>) -> Result<Response<()>, Status> {
        let req = request.into_inner();
        info!("Removing role {} from user {} in guild {}", req.role_id, req.user_id, req.guild_id);
        
        let pm = &self.plugin_manager;
        
        pm.remove_role_from_discord_user(&req.account_name, &req.guild_id, &req.user_id, &req.role_id).await
            .map_err(|e| Status::internal(format!("Failed to remove role: {}", e)))?;
        
        Ok(Response::new(()))
    }
    async fn get_member(&self, request: Request<GetMemberRequest>) -> Result<Response<GetMemberResponse>, Status> {
        let req = request.into_inner();
        debug!("Getting Discord member {} in guild {}", req.user_id, req.guild_id);
        
        // TODO: Implement member retrieval through Discord API
        // For now, return mock data
        let member = Member {
            user_id: req.user_id.clone(),
            username: "MockUser".to_string(),
            discriminator: "0001".to_string(),
            display_name: "MockUser".to_string(),
            avatar_url: String::new(),
            role_ids: vec![],
            joined_at: Some(prost_types::Timestamp {
                seconds: Utc::now().timestamp() - 86400 * 30,
                nanos: 0,
            }),
            is_owner: false,
            is_admin: false,
        };
        
        Ok(Response::new(GetMemberResponse {
            member: Some(member),
        }))
    }
    async fn list_members(&self, request: Request<ListMembersRequest>) -> Result<Response<ListMembersResponse>, Status> {
        let req = request.into_inner();
        debug!("Listing Discord members for guild: {}", req.guild_id);
        
        // TODO: Implement member listing through Discord API
        // For now, return empty list
        Ok(Response::new(ListMembersResponse {
            members: vec![],
            has_more: false,
        }))
    }
    async fn list_event_configs(&self, _: Request<ListEventConfigsRequest>) -> Result<Response<ListEventConfigsResponse>, Status> {
        debug!("Listing Discord event configs");
        
        let event_configs = self.plugin_manager.list_discord_event_configs().await
            .map_err(|e| Status::internal(format!("Failed to list event configs: {}", e)))?;
        
        // Convert to proto format
        let configs: Vec<EventConfig> = event_configs.into_iter()
            .map(|config| EventConfig {
                event_name: config.event_name,
                role_ids: vec![], // TODO: Get associated role IDs
                guild_id: config.guild_id,
                is_enabled: true,
            })
            .collect();
        
        Ok(Response::new(ListEventConfigsResponse {
            configs,
        }))
    }
    
    async fn add_event_config(&self, request: Request<AddEventConfigRequest>) -> Result<Response<()>, Status> {
        let req = request.into_inner();
        info!("Adding Discord event config: {} for guild {} channel {}", req.event_name, req.guild_id, req.channel_id);
        
        let credential_id = if req.credential_id.is_empty() {
            None
        } else {
            Some(Uuid::parse_str(&req.credential_id)
                .map_err(|e| Status::invalid_argument(format!("Invalid credential ID: {}", e)))?)
        };
        
        self.plugin_manager.add_discord_event_config(&req.event_name, &req.guild_id, &req.channel_id, credential_id).await
            .map_err(|e| Status::internal(format!("Failed to add event config: {}", e)))?;
        
        Ok(Response::new(()))
    }
    
    async fn remove_event_config(&self, request: Request<RemoveEventConfigRequest>) -> Result<Response<()>, Status> {
        let req = request.into_inner();
        info!("Removing Discord event config: {} for guild {} channel {}", req.event_name, req.guild_id, req.channel_id);
        
        let credential_id = if req.credential_id.is_empty() {
            None
        } else {
            Some(Uuid::parse_str(&req.credential_id)
                .map_err(|e| Status::invalid_argument(format!("Invalid credential ID: {}", e)))?)
        };
        
        self.plugin_manager.remove_discord_event_config(&req.event_name, &req.guild_id, &req.channel_id, credential_id).await
            .map_err(|e| Status::internal(format!("Failed to remove event config: {}", e)))?;
        
        Ok(Response::new(()))
    }
    async fn add_event_role(&self, request: Request<AddEventRoleRequest>) -> Result<Response<()>, Status> {
        let req = request.into_inner();
        info!("Adding event role {} for event {}", req.role_id, req.event_name);
        
        self.plugin_manager.add_discord_event_role(&req.event_name, &req.role_id).await
            .map_err(|e| Status::internal(format!("Failed to add event role: {}", e)))?;
        
        Ok(Response::new(()))
    }
    async fn remove_event_role(&self, request: Request<RemoveEventRoleRequest>) -> Result<Response<()>, Status> {
        let req = request.into_inner();
        info!("Removing event role {} for event {}", req.role_id, req.event_name);
        
        self.plugin_manager.remove_discord_event_role(&req.event_name, &req.role_id).await
            .map_err(|e| Status::internal(format!("Failed to remove event role: {}", e)))?;
        
        Ok(Response::new(()))
    }
    async fn set_live_role(&self, request: Request<SetLiveRoleRequest>) -> Result<Response<()>, Status> {
        let req = request.into_inner();
        info!("Setting live role {} for guild {}", req.role_id, req.guild_id);
        
        self.plugin_manager.set_discord_live_role(&req.guild_id, &req.role_id).await
            .map_err(|e| Status::internal(format!("Failed to set live role: {}", e)))?;
        
        Ok(Response::new(()))
    }
    async fn delete_live_role(&self, request: Request<DeleteLiveRoleRequest>) -> Result<Response<()>, Status> {
        let req = request.into_inner();
        info!("Deleting live role for guild {}", req.guild_id);
        
        self.plugin_manager.delete_discord_live_role(&req.guild_id).await
            .map_err(|e| Status::internal(format!("Failed to delete live role: {}", e)))?;
        
        Ok(Response::new(()))
    }
    async fn list_live_roles(&self, _: Request<ListLiveRolesRequest>) -> Result<Response<ListLiveRolesResponse>, Status> {
        debug!("Listing Discord live roles");
        
        let live_roles = self.plugin_manager.list_discord_live_roles().await
            .map_err(|e| Status::internal(format!("Failed to list live roles: {}", e)))?;
        
        // Convert to proto format
        let roles: Vec<LiveRole> = live_roles.into_iter()
            .map(|role| LiveRole {
                guild_id: role.guild_id,
                role_id: role.role_id,
                role_name: String::new(), // TODO: Look up role name from Discord API
                created_at: Some(prost_types::Timestamp {
                    seconds: role.created_at.timestamp(),
                    nanos: role.created_at.timestamp_subsec_nanos() as i32,
                }),
            })
            .collect();
        
        Ok(Response::new(ListLiveRolesResponse {
            roles,
        }))
    }
    async fn upsert_discord_account(&self, request: Request<UpsertDiscordAccountRequest>) -> Result<Response<UpsertDiscordAccountResponse>, Status> {
        let req = request.into_inner();
        info!("Upserting Discord account: {}", req.account_name);
        
        let credential_id = if req.credential_id.is_empty() {
            None
        } else {
            Some(Uuid::parse_str(&req.credential_id)
                .map_err(|e| Status::invalid_argument(format!("Invalid credential ID: {}", e)))?)
        };
        
        let discord_id = if req.discord_id.is_empty() { None } else { Some(req.discord_id.as_str()) };
        self.plugin_manager.upsert_discord_account(&req.account_name, credential_id, discord_id).await
            .map_err(|e| Status::internal(format!("Failed to upsert account: {}", e)))?;
        
        // Since we don't have a get_discord_account method, we'll assume success
        let account = Some(maowbot_common::models::discord::DiscordAccountRecord {
            account_name: req.account_name.clone(),
            credential_id,
            discord_id: discord_id.map(|s| s.to_string()),
            is_active: true,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        });
        
        if let Some(acc) = account {
            Ok(Response::new(UpsertDiscordAccountResponse {
                account_id: acc.account_name, // Using account_name as the ID since that's the primary key
                was_created: true, // We don't track this in the repository, so assume created
            }))
        } else {
            Err(Status::internal("Failed to retrieve created account"))
        }
    }
    type StreamDiscordEventsStream = tonic::codec::Streaming<DiscordEvent>;
    async fn stream_discord_events(&self, _: Request<StreamDiscordEventsRequest>) -> Result<Response<Self::StreamDiscordEventsStream>, Status> {
        Err(Status::unimplemented("Not implemented"))
    }
}