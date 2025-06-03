use crate::{GrpcClient, CommandResult, CommandError};
use maowbot_proto::maowbot::services::{
    SetLiveRoleRequest, DeleteLiveRoleRequest, ListLiveRolesRequest,
    SendDiscordMessageRequest, GetGuildRequest, ListGuildsRequest,
    GetChannelRequest, ListChannelsRequest,
    GetMemberRequest, ListMembersRequest,
    LiveRole, Guild, Channel, Member,
};

// Result structures
pub struct SetLiveRoleResult {
    // SetLiveRole returns Empty, so no data
}

pub struct ListLiveRolesResult {
    pub live_roles: Vec<LiveRole>,
}

pub struct SendDiscordMessageResult {
    pub message_id: String,
}

pub struct GetGuildResult {
    pub guild: Guild,
}

pub struct ListGuildsResult {
    pub guilds: Vec<Guild>,
}

pub struct GetChannelResult {
    pub channel: Channel,
}

pub struct ListChannelsResult {
    pub channels: Vec<Channel>,
}

pub struct GetMemberResult {
    pub member: Member,
}

pub struct ListMembersResult {
    pub members: Vec<Member>,
    pub has_more: bool,
}

// Command handlers
pub struct DiscordCommands;

impl DiscordCommands {
    pub async fn set_live_role(
        client: &GrpcClient,
        guild_id: &str,
        role_id: &str,
    ) -> Result<CommandResult<SetLiveRoleResult>, CommandError> {
        let request = SetLiveRoleRequest {
            guild_id: guild_id.to_string(),
            role_id: role_id.to_string(),
        };

        client.discord.clone()
            .set_live_role(request)
            .await
            .map_err(|e| CommandError::GrpcError(e.to_string()))?;

        Ok(CommandResult {
            data: SetLiveRoleResult {},
            warnings: vec![],
        })
    }

    pub async fn delete_live_role(
        client: &GrpcClient,
        guild_id: &str,
    ) -> Result<CommandResult<()>, CommandError> {
        let request = DeleteLiveRoleRequest {
            guild_id: guild_id.to_string(),
        };

        client.discord.clone()
            .delete_live_role(request)
            .await
            .map_err(|e| CommandError::GrpcError(e.to_string()))?;

        Ok(CommandResult {
            data: (),
            warnings: vec![],
        })
    }

    pub async fn list_live_roles(
        client: &GrpcClient,
    ) -> Result<CommandResult<ListLiveRolesResult>, CommandError> {
        let request = ListLiveRolesRequest {
            guild_id: String::new(), // Empty for all
        };

        let response = client.discord.clone()
            .list_live_roles(request)
            .await
            .map_err(|e| CommandError::GrpcError(e.to_string()))?;

        Ok(CommandResult {
            data: ListLiveRolesResult {
                live_roles: response.into_inner().roles,
            },
            warnings: vec![],
        })
    }

    pub async fn send_message(
        client: &GrpcClient,
        account_name: &str,
        channel_id: &str,
        content: &str,
    ) -> Result<CommandResult<SendDiscordMessageResult>, CommandError> {
        let request = SendDiscordMessageRequest {
            account_name: account_name.to_string(),
            channel_id: channel_id.to_string(),
            content: content.to_string(),
            embeds: vec![],
            reference: None,
            mentions: vec![],
            tts: false,
        };

        let response = client.discord.clone()
            .send_message(request)
            .await
            .map_err(|e| CommandError::GrpcError(e.to_string()))?;

        Ok(CommandResult {
            data: SendDiscordMessageResult {
                message_id: response.into_inner().message_id,
            },
            warnings: vec![],
        })
    }

    pub async fn get_guild(
        client: &GrpcClient,
        account_name: &str,
        guild_id: &str,
    ) -> Result<CommandResult<GetGuildResult>, CommandError> {
        let request = GetGuildRequest {
            account_name: account_name.to_string(),
            guild_id: guild_id.to_string(),
        };

        let response = client.discord.clone()
            .get_guild(request)
            .await
            .map_err(|e| CommandError::GrpcError(e.to_string()))?;

        let guild = response.into_inner().guild
            .ok_or_else(|| CommandError::DataError("No guild returned".to_string()))?;

        Ok(CommandResult {
            data: GetGuildResult { guild },
            warnings: vec![],
        })
    }

    pub async fn list_guilds(
        client: &GrpcClient,
        account_name: &str,
    ) -> Result<CommandResult<ListGuildsResult>, CommandError> {
        let request = ListGuildsRequest {
            account_name: account_name.to_string(),
        };

        let response = client.discord.clone()
            .list_guilds(request)
            .await
            .map_err(|e| CommandError::GrpcError(e.to_string()))?;

        Ok(CommandResult {
            data: ListGuildsResult {
                guilds: response.into_inner().guilds,
            },
            warnings: vec![],
        })
    }

    pub async fn get_channel(
        client: &GrpcClient,
        account_name: &str,
        channel_id: &str,
    ) -> Result<CommandResult<GetChannelResult>, CommandError> {
        let request = GetChannelRequest {
            account_name: account_name.to_string(),
            channel_id: channel_id.to_string(),
        };

        let response = client.discord.clone()
            .get_channel(request)
            .await
            .map_err(|e| CommandError::GrpcError(e.to_string()))?;

        let channel = response.into_inner().channel
            .ok_or_else(|| CommandError::DataError("No channel returned".to_string()))?;

        Ok(CommandResult {
            data: GetChannelResult { channel },
            warnings: vec![],
        })
    }

    pub async fn list_channels(
        client: &GrpcClient,
        account_name: &str,
        guild_id: &str,
    ) -> Result<CommandResult<ListChannelsResult>, CommandError> {
        let request = ListChannelsRequest {
            account_name: account_name.to_string(),
            guild_id: guild_id.to_string(),
            channel_types: vec![],
        };

        let response = client.discord.clone()
            .list_channels(request)
            .await
            .map_err(|e| CommandError::GrpcError(e.to_string()))?;

        Ok(CommandResult {
            data: ListChannelsResult {
                channels: response.into_inner().channels,
            },
            warnings: vec![],
        })
    }

    pub async fn get_member(
        client: &GrpcClient,
        account_name: &str,
        guild_id: &str,
        user_id: &str,
    ) -> Result<CommandResult<GetMemberResult>, CommandError> {
        let request = GetMemberRequest {
            account_name: account_name.to_string(),
            guild_id: guild_id.to_string(),
            user_id: user_id.to_string(),
        };

        let response = client.discord.clone()
            .get_member(request)
            .await
            .map_err(|e| CommandError::GrpcError(e.to_string()))?;

        let member = response.into_inner().member
            .ok_or_else(|| CommandError::DataError("No member returned".to_string()))?;

        Ok(CommandResult {
            data: GetMemberResult { member },
            warnings: vec![],
        })
    }

    pub async fn list_members(
        client: &GrpcClient,
        account_name: &str,
        guild_id: &str,
        limit: i32,
    ) -> Result<CommandResult<ListMembersResult>, CommandError> {
        let request = ListMembersRequest {
            account_name: account_name.to_string(),
            guild_id: guild_id.to_string(),
            limit,
            after: String::new(),
        };

        let response = client.discord.clone()
            .list_members(request)
            .await
            .map_err(|e| CommandError::GrpcError(e.to_string()))?;

        let resp = response.into_inner();

        Ok(CommandResult {
            data: ListMembersResult {
                members: resp.members,
                has_more: resp.has_more,
            },
            warnings: vec![],
        })
    }
}