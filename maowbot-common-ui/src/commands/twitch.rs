use crate::{GrpcClient, CommandResult, CommandError};
use maowbot_proto::maowbot::services::{
    JoinChannelRequest, PartChannelRequest, SendMessageRequest,
    GetJoinedChannelsRequest, ChannelMembership,
    GetChannelInfoRequest, GetStreamInfoRequest,
    GetFollowAgeRequest, StreamInfo, ChannelInfo,
};

// Result structures
pub struct SendMessageResult {
    pub message_id: String,
    pub sent_at: Option<maowbot_proto::prost_types::Timestamp>,
}

pub struct GetJoinedChannelsResult {
    pub channels: Vec<ChannelMembership>,
}

pub struct GetChannelInfoResult {
    pub channel: ChannelInfo,
}

pub struct GetStreamInfoResult {
    pub stream: Option<StreamInfo>,
}

pub struct GetFollowAgeResult {
    pub is_following: bool,
    pub followed_at: Option<maowbot_proto::prost_types::Timestamp>,
    pub follow_duration: String,
}

// Command handlers
pub struct TwitchCommands;

impl TwitchCommands {
    pub async fn join_channel(
        client: &GrpcClient,
        account_name: &str,
        channel: &str,
    ) -> Result<CommandResult<()>, CommandError> {
        let request = JoinChannelRequest {
            account_name: account_name.to_string(),
            channel: channel.to_string(),
        };

        client.twitch.clone()
            .join_channel(request)
            .await
            .map_err(|e| CommandError::GrpcError(e.to_string()))?;

        Ok(CommandResult {
            data: (),
            warnings: vec![],
        })
    }

    pub async fn part_channel(
        client: &GrpcClient,
        account_name: &str,
        channel: &str,
    ) -> Result<CommandResult<()>, CommandError> {
        let request = PartChannelRequest {
            account_name: account_name.to_string(),
            channel: channel.to_string(),
        };

        client.twitch.clone()
            .part_channel(request)
            .await
            .map_err(|e| CommandError::GrpcError(e.to_string()))?;

        Ok(CommandResult {
            data: (),
            warnings: vec![],
        })
    }

    pub async fn send_message(
        client: &GrpcClient,
        account_name: &str,
        channel: &str,
        text: &str,
    ) -> Result<CommandResult<SendMessageResult>, CommandError> {
        let request = SendMessageRequest {
            account_name: account_name.to_string(),
            channel: channel.to_string(),
            text: text.to_string(),
            is_action: false,
            reply_to_message_id: String::new(),
        };

        let response = client.twitch.clone()
            .send_message(request)
            .await
            .map_err(|e| CommandError::GrpcError(e.to_string()))?;

        let resp = response.into_inner();
        Ok(CommandResult {
            data: SendMessageResult {
                message_id: resp.message_id,
                sent_at: resp.sent_at,
            },
            warnings: vec![],
        })
    }

    pub async fn get_joined_channels(
        client: &GrpcClient,
        account_name: &str,
    ) -> Result<CommandResult<GetJoinedChannelsResult>, CommandError> {
        let request = GetJoinedChannelsRequest {
            account_name: account_name.to_string(),
        };

        let response = client.twitch.clone()
            .get_joined_channels(request)
            .await
            .map_err(|e| CommandError::GrpcError(e.to_string()))?;

        Ok(CommandResult {
            data: GetJoinedChannelsResult {
                channels: response.into_inner().channels,
            },
            warnings: vec![],
        })
    }

    pub async fn get_channel_info(
        client: &GrpcClient,
        channel: &str,
    ) -> Result<CommandResult<GetChannelInfoResult>, CommandError> {
        let request = GetChannelInfoRequest {
            channel: channel.to_string(),
        };

        let response = client.twitch.clone()
            .get_channel_info(request)
            .await
            .map_err(|e| CommandError::GrpcError(e.to_string()))?;

        let channel = response.into_inner().channel
            .ok_or_else(|| CommandError::DataError("No channel info returned".to_string()))?;

        Ok(CommandResult {
            data: GetChannelInfoResult { channel },
            warnings: vec![],
        })
    }

    pub async fn get_stream_info(
        client: &GrpcClient,
        channel: &str,
    ) -> Result<CommandResult<GetStreamInfoResult>, CommandError> {
        let request = GetStreamInfoRequest {
            channel: channel.to_string(),
        };

        let response = client.twitch.clone()
            .get_stream_info(request)
            .await
            .map_err(|e| CommandError::GrpcError(e.to_string()))?;

        Ok(CommandResult {
            data: GetStreamInfoResult {
                stream: response.into_inner().stream,
            },
            warnings: vec![],
        })
    }

    pub async fn get_follow_age(
        client: &GrpcClient,
        channel: &str,
        user: &str,
    ) -> Result<CommandResult<GetFollowAgeResult>, CommandError> {
        let request = GetFollowAgeRequest {
            channel: channel.to_string(),
            user: user.to_string(),
        };

        let response = client.twitch.clone()
            .get_follow_age(request)
            .await
            .map_err(|e| CommandError::GrpcError(e.to_string()))?;

        let resp = response.into_inner();
        Ok(CommandResult {
            data: GetFollowAgeResult {
                is_following: resp.is_following,
                followed_at: resp.followed_at,
                follow_duration: resp.follow_duration,
            },
            warnings: vec![],
        })
    }
}