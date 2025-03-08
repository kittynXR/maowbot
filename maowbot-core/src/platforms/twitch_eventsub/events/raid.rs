// File: maowbot-core/src/platforms/twitch_eventsub/events/raid.rs

use serde::Deserialize;

/// "channel.raid" event
#[derive(Debug, Clone, Deserialize)]
pub struct ChannelRaid {
    pub from_broadcaster_user_id: String,
    pub from_broadcaster_user_login: String,
    pub from_broadcaster_user_name: String,
    pub to_broadcaster_user_id: String,
    pub to_broadcaster_user_login: String,
    pub to_broadcaster_user_name: String,
    pub viewers: u64,
}
