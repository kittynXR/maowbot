// File: maowbot-core/src/platforms/twitch_eventsub/events/shared_chat.rs

use serde::Deserialize;

/// "channel.shared_chat.begin" event
#[derive(Debug, Clone, Deserialize)]
pub struct ChannelSharedChatBegin {
    pub session_id: String,
    pub broadcaster_user_id: String,
    pub broadcaster_user_login: String,
    pub broadcaster_user_name: String,
    pub host_broadcaster_user_id: String,
    pub host_broadcaster_user_login: String,
    pub host_broadcaster_user_name: String,
    pub participants: Vec<Participant>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ChannelSharedChatUpdate {
    pub session_id: String,
    pub broadcaster_user_id: String,
    pub broadcaster_user_login: String,
    pub broadcaster_user_name: String,
    pub host_broadcaster_user_id: String,
    pub host_broadcaster_user_login: String,
    pub host_broadcaster_user_name: String,
    pub participants: Vec<Participant>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ChannelSharedChatEnd {
    pub session_id: String,
    pub broadcaster_user_id: String,
    pub broadcaster_user_login: String,
    pub broadcaster_user_name: String,
    pub host_broadcaster_user_id: String,
    pub host_broadcaster_user_login: String,
    pub host_broadcaster_user_name: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Participant {
    pub broadcaster_user_id: String,
    pub broadcaster_user_login: String,
    pub broadcaster_user_name: String,
}
