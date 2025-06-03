use tonic::{Request, Response, Status};
use maowbot_proto::maowbot::services::{twitch_service_server::TwitchService, *};
use maowbot_core::platforms::manager::PlatformManager;
use maowbot_common::traits::api::TwitchApi;
use std::sync::Arc;
use chrono::Utc;
use tracing::{info, error, debug};
use prost_types;
use uuid::Uuid;

pub struct TwitchServiceImpl {
    platform_manager: Arc<PlatformManager>,
}

impl TwitchServiceImpl {
    pub fn new(platform_manager: Arc<PlatformManager>) -> Self {
        Self {
            platform_manager,
        }
    }
}

#[tonic::async_trait]
impl TwitchService for TwitchServiceImpl {
    async fn join_channel(&self, request: Request<JoinChannelRequest>) -> Result<Response<()>, Status> {
        let req = request.into_inner();
        info!("Joining Twitch channel: {} on account: {}", req.channel, req.account_name);
        
        let pm = &self.platform_manager;
        
        // Ensure channel name has # prefix
        let channel = if req.channel.starts_with('#') {
            req.channel
        } else {
            format!("#{}", req.channel)
        };
        
        pm.join_twitch_irc_channel(&req.account_name, &channel).await
            .map_err(|e| Status::internal(format!("Failed to join channel: {}", e)))?;
        
        Ok(Response::new(()))
    }
    async fn part_channel(&self, request: Request<PartChannelRequest>) -> Result<Response<()>, Status> {
        let req = request.into_inner();
        info!("Parting Twitch channel: {} on account: {}", req.channel, req.account_name);
        
        let pm = &self.platform_manager;
        
        // Ensure channel name has # prefix
        let channel = if req.channel.starts_with('#') {
            req.channel
        } else {
            format!("#{}", req.channel)
        };
        
        pm.part_twitch_irc_channel(&req.account_name, &channel).await
            .map_err(|e| Status::internal(format!("Failed to part channel: {}", e)))?;
        
        Ok(Response::new(()))
    }
    async fn send_message(&self, request: Request<SendMessageRequest>) -> Result<Response<SendMessageResponse>, Status> {
        let req = request.into_inner();
        debug!("Sending message to Twitch channel: {}", req.channel);
        
        let pm = &self.platform_manager;
        
        // Ensure channel name has # prefix
        let channel = if req.channel.starts_with('#') {
            req.channel
        } else {
            format!("#{}", req.channel)
        };
        
        // TODO: Handle reply_to_message_id and is_action
        pm.send_twitch_irc_message(&req.account_name, &channel, &req.text).await
            .map_err(|e| Status::internal(format!("Failed to send message: {}", e)))?;
        
        // Generate a mock message ID
        let message_id = Uuid::new_v4().to_string();
        let sent_at = Utc::now();
        
        Ok(Response::new(SendMessageResponse {
            message_id,
            sent_at: Some(prost_types::Timestamp {
                seconds: sent_at.timestamp(),
                nanos: sent_at.timestamp_subsec_nanos() as i32,
            }),
        }))
    }
    async fn get_joined_channels(&self, request: Request<GetJoinedChannelsRequest>) -> Result<Response<GetJoinedChannelsResponse>, Status> {
        let req = request.into_inner();
        debug!("Getting joined channels for account: {}", req.account_name);
        
        // TODO: Track joined channels in platform manager
        // For now, return empty list
        Ok(Response::new(GetJoinedChannelsResponse {
            channels: vec![],
        }))
    }
    async fn ban_user(&self, request: Request<BanUserRequest>) -> Result<Response<()>, Status> {
        let req = request.into_inner();
        info!("Banning user {} in channel {} - reason: {}", req.user_id, req.channel, req.reason);
        
        // TODO: Implement ban through Twitch API
        Err(Status::unimplemented("Ban functionality not yet implemented"))
    }
    async fn unban_user(&self, request: Request<UnbanUserRequest>) -> Result<Response<()>, Status> {
        let req = request.into_inner();
        info!("Unbanning user {} in channel {}", req.user_id, req.channel);
        
        // TODO: Implement unban through Twitch API
        Err(Status::unimplemented("Unban functionality not yet implemented"))
    }
    async fn timeout_user(&self, request: Request<TimeoutUserRequest>) -> Result<Response<()>, Status> {
        let req = request.into_inner();
        info!("Timing out user {} for {} seconds in channel {} - reason: {}", 
              req.user_id, req.duration_seconds, req.channel, req.reason);
        
        let pm = &self.platform_manager;
        
        // Ensure channel name has # prefix
        let channel = if req.channel.starts_with('#') {
            req.channel.clone()
        } else {
            format!("#{}", req.channel)
        };
        
        let reason = if req.reason.is_empty() { None } else { Some(req.reason.as_str()) };
        
        pm.timeout_twitch_user(&req.account_name, &channel, &req.user_id, req.duration_seconds as u32, reason).await
            .map_err(|e| Status::internal(format!("Failed to timeout user: {}", e)))?;
        
        Ok(Response::new(()))
    }
    async fn delete_message(&self, request: Request<DeleteMessageRequest>) -> Result<Response<()>, Status> {
        let req = request.into_inner();
        info!("Deleting message {} in channel {}", req.message_id, req.channel);
        
        // TODO: Implement message deletion through Twitch API
        Err(Status::unimplemented("Message deletion not yet implemented"))
    }
    async fn get_channel_info(&self, request: Request<GetChannelInfoRequest>) -> Result<Response<GetChannelInfoResponse>, Status> {
        let req = request.into_inner();
        debug!("Getting channel info for: {}", req.channel);
        
        // TODO: Implement channel info retrieval through Twitch API
        // For now, return mock data
        let channel_info = ChannelInfo {
            channel_id: "123456789".to_string(),
            channel_name: req.channel.clone(),
            display_name: req.channel,
            game_name: "Just Chatting".to_string(),
            game_id: "509658".to_string(),
            title: "Mock stream title".to_string(),
            language: "en".to_string(),
            tags: vec![],
            is_mature: false,
        };
        
        Ok(Response::new(GetChannelInfoResponse {
            channel: Some(channel_info),
        }))
    }
    async fn update_channel_info(&self, request: Request<UpdateChannelInfoRequest>) -> Result<Response<()>, Status> {
        let req = request.into_inner();
        info!("Updating channel info for: {}", req.channel);
        
        // TODO: Implement channel update through Twitch API
        Err(Status::unimplemented("Channel update not yet implemented"))
    }
    async fn get_stream_info(&self, request: Request<GetStreamInfoRequest>) -> Result<Response<GetStreamInfoResponse>, Status> {
        let req = request.into_inner();
        debug!("Getting stream info for: {}", req.channel);
        
        // TODO: Implement stream info retrieval through Twitch API
        // For now, return mock data
        let stream_info = StreamInfo {
            stream_id: "42949012544".to_string(),
            is_live: true,
            started_at: Some(prost_types::Timestamp {
                seconds: Utc::now().timestamp() - 3600,
                nanos: 0,
            }),
            viewer_count: 42,
            title: "Mock stream title".to_string(),
            game_name: "Just Chatting".to_string(),
            thumbnail_url: "https://example.com/thumbnail.jpg".to_string(),
        };
        
        Ok(Response::new(GetStreamInfoResponse {
            stream: Some(stream_info),
        }))
    }
    async fn get_followers(&self, request: Request<GetFollowersRequest>) -> Result<Response<GetFollowersResponse>, Status> {
        let req = request.into_inner();
        debug!("Getting followers for channel: {}", req.channel);
        
        // TODO: Implement follower retrieval through Twitch API
        // For now, return empty list
        Ok(Response::new(GetFollowersResponse {
            followers: vec![],
            total_count: 0,
            page: None,
        }))
    }
    async fn get_follow_age(&self, request: Request<GetFollowAgeRequest>) -> Result<Response<GetFollowAgeResponse>, Status> {
        let req = request.into_inner();
        debug!("Getting follow age for user {} in channel {}", req.user, req.channel);
        
        // TODO: Implement follow age calculation through Twitch API
        // For now, return mock data
        Ok(Response::new(GetFollowAgeResponse {
            is_following: true,
            followed_at: Some(prost_types::Timestamp {
                seconds: Utc::now().timestamp() - 86400 * 30, // 30 days ago
                nanos: 0,
            }),
            follow_duration: "30 days".to_string(),
        }))
    }
    async fn get_subscribers(&self, request: Request<GetSubscribersRequest>) -> Result<Response<GetSubscribersResponse>, Status> {
        let req = request.into_inner();
        debug!("Getting subscribers for channel: {}", req.channel);
        
        // TODO: Implement subscriber retrieval through Twitch API
        // For now, return empty list
        Ok(Response::new(GetSubscribersResponse {
            subscribers: vec![],
            total_count: 0,
            point_count: 0,
            page: None,
        }))
    }
    async fn check_subscription(&self, request: Request<CheckSubscriptionRequest>) -> Result<Response<CheckSubscriptionResponse>, Status> {
        let req = request.into_inner();
        debug!("Checking subscription for user {} in channel {}", req.user, req.channel);
        
        // TODO: Implement subscription check through Twitch API
        // For now, return not subscribed
        Ok(Response::new(CheckSubscriptionResponse {
            is_subscribed: false,
            subscription: None,
        }))
    }
    async fn get_channel_point_rewards(&self, request: Request<GetChannelPointRewardsRequest>) -> Result<Response<GetChannelPointRewardsResponse>, Status> {
        let req = request.into_inner();
        debug!("Getting channel point rewards for channel: {}", req.channel);
        
        // TODO: Implement reward retrieval through Twitch API
        // For now, return empty list
        Ok(Response::new(GetChannelPointRewardsResponse {
            rewards: vec![],
        }))
    }
    async fn create_channel_point_reward(&self, request: Request<CreateChannelPointRewardRequest>) -> Result<Response<CreateChannelPointRewardResponse>, Status> {
        let req = request.into_inner();
        let reward = req.reward.ok_or_else(|| Status::invalid_argument("Reward data is required"))?;
        info!("Creating channel point reward: {} for channel: {}", reward.title, req.channel);
        
        // TODO: Implement reward creation through Twitch API
        Err(Status::unimplemented("Channel point reward creation not yet implemented"))
    }
    async fn update_channel_point_reward(&self, request: Request<UpdateChannelPointRewardRequest>) -> Result<Response<()>, Status> {
        let req = request.into_inner();
        info!("Updating channel point reward: {} for channel: {}", req.reward_id, req.channel);
        
        // TODO: Implement reward update through Twitch API
        Err(Status::unimplemented("Channel point reward update not yet implemented"))
    }
    async fn delete_channel_point_reward(&self, request: Request<DeleteChannelPointRewardRequest>) -> Result<Response<()>, Status> {
        let req = request.into_inner();
        info!("Deleting channel point reward: {} for channel: {}", req.reward_id, req.channel);
        
        // TODO: Implement reward deletion through Twitch API
        Err(Status::unimplemented("Channel point reward deletion not yet implemented"))
    }
    async fn fulfill_redemption(&self, request: Request<FulfillRedemptionRequest>) -> Result<Response<()>, Status> {
        let req = request.into_inner();
        info!("Fulfilling redemption: {} for reward: {} in channel: {}", 
              req.redemption_id, req.reward_id, req.channel);
        
        // TODO: Implement redemption fulfillment through Twitch API
        Err(Status::unimplemented("Redemption fulfillment not yet implemented"))
    }
    type StreamTwitchEventsStream = tonic::codec::Streaming<TwitchEvent>;
    async fn stream_twitch_events(&self, _: Request<StreamTwitchEventsRequest>) -> Result<Response<Self::StreamTwitchEventsStream>, Status> {
        // TODO: Implement Twitch event streaming
        Err(Status::unimplemented("Twitch event streaming not yet implemented"))
    }
    async fn batch_send_messages(&self, request: Request<BatchSendMessagesRequest>) -> Result<Response<BatchSendMessagesResponse>, Status> {
        let req = request.into_inner();
        info!("Batch sending {} messages", req.messages.len());
        
        let pm = &self.platform_manager;
        let mut results = Vec::new();
        
        for msg in req.messages {
            // Ensure channel name has # prefix
            let channel = if msg.channel.starts_with('#') {
                msg.channel.clone()
            } else {
                format!("#{}", msg.channel)
            };
            
            let result = match pm.send_twitch_irc_message(&req.account_name, &channel, &msg.text).await {
                Ok(_) => SendResult {
                    channel: msg.channel,
                    success: true,
                    message_id: Uuid::new_v4().to_string(),
                    error_message: String::new(),
                },
                Err(e) => SendResult {
                    channel: msg.channel,
                    success: false,
                    message_id: String::new(),
                    error_message: format!("{}", e),
                },
            };
            results.push(result);
        }
        
        let success_count = results.iter().filter(|r| r.success).count() as i32;
        let failure_count = results.len() as i32 - success_count;
        
        Ok(Response::new(BatchSendMessagesResponse {
            results,
            success_count,
            failure_count,
        }))
    }
}