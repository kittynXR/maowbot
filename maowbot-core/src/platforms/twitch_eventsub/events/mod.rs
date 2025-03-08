// File: maowbot-core/src/platforms/twitch_eventsub/events/mod.rs

pub mod base;
pub mod bits;
pub mod channel_follow_update;
pub mod chat;
pub mod shared_chat;
pub mod subscription;
pub mod ban_unban;
pub mod hype_train;
pub mod raid;
pub mod shoutout;
pub mod channel_points;

pub use base::*;
pub use bits::*;
pub use channel_follow_update::*;
pub use chat::*;
pub use shared_chat::*;
pub use subscription::*;
pub use ban_unban::*;
pub use hype_train::*;
pub use raid::*;
pub use shoutout::*;
pub use channel_points::*;

// ------------------------------------------------------------------------
// The parse_twitch_notification function has been moved here.
// It references all the sub-types from our newly split modules.
// ------------------------------------------------------------------------

use crate::eventbus::TwitchEventSubData;

/// Helper function to parse from JSON into a known event type. Returns None if unknown.
pub fn parse_twitch_notification(
    sub_type: &str,
    event_json: &serde_json::Value
) -> Option<TwitchEventSubData> {
    // We'll do a match on subscription.type
    match sub_type {
        "channel.bits.use" => {
            serde_json::from_value::<ChannelBitsUse>(event_json.clone()).ok()
                .map(TwitchEventSubData::ChannelBitsUse)
        }
        "channel.update" => {
            serde_json::from_value::<ChannelUpdate>(event_json.clone()).ok()
                .map(TwitchEventSubData::ChannelUpdate)
        }
        "channel.follow" => {
            serde_json::from_value::<ChannelFollow>(event_json.clone()).ok()
                .map(TwitchEventSubData::ChannelFollow)
        }
        "channel.ad_break.begin" => {
            serde_json::from_value::<ChannelAdBreakBegin>(event_json.clone()).ok()
                .map(TwitchEventSubData::ChannelAdBreakBegin)
        }
        "channel.chat.notification" => {
            serde_json::from_value::<ChannelChatNotification>(event_json.clone()).ok()
                .map(TwitchEventSubData::ChannelChatNotification)
        }
        "channel.shared_chat.begin" => {
            serde_json::from_value::<ChannelSharedChatBegin>(event_json.clone()).ok()
                .map(TwitchEventSubData::ChannelSharedChatBegin)
        }
        "channel.shared_chat.update" => {
            serde_json::from_value::<ChannelSharedChatUpdate>(event_json.clone()).ok()
                .map(TwitchEventSubData::ChannelSharedChatUpdate)
        }
        "channel.shared_chat.end" => {
            serde_json::from_value::<ChannelSharedChatEnd>(event_json.clone()).ok()
                .map(TwitchEventSubData::ChannelSharedChatEnd)
        }
        "channel.subscribe" => {
            serde_json::from_value::<ChannelSubscribe>(event_json.clone()).ok()
                .map(TwitchEventSubData::ChannelSubscribe)
        }
        "channel.subscription.end" => {
            serde_json::from_value::<ChannelSubscriptionEnd>(event_json.clone()).ok()
                .map(TwitchEventSubData::ChannelSubscriptionEnd)
        }
        "channel.subscription.gift" => {
            serde_json::from_value::<ChannelSubscriptionGift>(event_json.clone()).ok()
                .map(TwitchEventSubData::ChannelSubscriptionGift)
        }
        "channel.subscription.message" => {
            serde_json::from_value::<ChannelSubscriptionMessage>(event_json.clone()).ok()
                .map(TwitchEventSubData::ChannelSubscriptionMessage)
        }
        "channel.cheer" => {
            serde_json::from_value::<ChannelCheer>(event_json.clone()).ok()
                .map(TwitchEventSubData::ChannelCheer)
        }
        "channel.raid" => {
            serde_json::from_value::<ChannelRaid>(event_json.clone()).ok()
                .map(TwitchEventSubData::ChannelRaid)
        }
        "channel.ban" => {
            serde_json::from_value::<ChannelBan>(event_json.clone()).ok()
                .map(TwitchEventSubData::ChannelBan)
        }
        "channel.unban" => {
            serde_json::from_value::<ChannelUnban>(event_json.clone()).ok()
                .map(TwitchEventSubData::ChannelUnban)
        }
        "channel.unban_request.create" => {
            serde_json::from_value::<ChannelUnbanRequestCreate>(event_json.clone()).ok()
                .map(TwitchEventSubData::ChannelUnbanRequestCreate)
        }
        "channel.unban_request.resolve" => {
            serde_json::from_value::<ChannelUnbanRequestResolve>(event_json.clone()).ok()
                .map(TwitchEventSubData::ChannelUnbanRequestResolve)
        }
        "channel.hype_train.begin" => {
            serde_json::from_value::<ChannelHypeTrainBegin>(event_json.clone()).ok()
                .map(TwitchEventSubData::ChannelHypeTrainBegin)
        }
        "channel.hype_train.progress" => {
            serde_json::from_value::<ChannelHypeTrainProgress>(event_json.clone()).ok()
                .map(TwitchEventSubData::ChannelHypeTrainProgress)
        }
        "channel.hype_train.end" => {
            serde_json::from_value::<ChannelHypeTrainEnd>(event_json.clone()).ok()
                .map(TwitchEventSubData::ChannelHypeTrainEnd)
        }
        "channel.shoutout.create" => {
            serde_json::from_value::<ChannelShoutoutCreate>(event_json.clone()).ok()
                .map(TwitchEventSubData::ChannelShoutoutCreate)
        }
        "channel.shoutout.receive" => {
            serde_json::from_value::<ChannelShoutoutReceive>(event_json.clone()).ok()
                .map(TwitchEventSubData::ChannelShoutoutReceive)
        }
        "channel.channel_points_automatic_reward_redemption.add" => {
            serde_json::from_value::<ChannelPointsAutomaticRewardRedemptionAddV2>(event_json.clone())
                .ok()
                .map(TwitchEventSubData::ChannelPointsAutomaticRewardRedemptionAddV2)
        }
        "channel.channel_points_custom_reward.add" => {
            serde_json::from_value::<ChannelPointsCustomReward>(event_json.clone())
                .ok()
                .map(TwitchEventSubData::ChannelPointsCustomRewardAdd)
        }
        "channel.channel_points_custom_reward.update" => {
            serde_json::from_value::<ChannelPointsCustomReward>(event_json.clone())
                .ok()
                .map(TwitchEventSubData::ChannelPointsCustomRewardUpdate)
        }
        "channel.channel_points_custom_reward.remove" => {
            serde_json::from_value::<ChannelPointsCustomReward>(event_json.clone())
                .ok()
                .map(TwitchEventSubData::ChannelPointsCustomRewardRemove)
        }
        "channel.channel_points_custom_reward_redemption.add" => {
            serde_json::from_value::<ChannelPointsCustomRewardRedemption>(event_json.clone())
                .ok()
                .map(TwitchEventSubData::ChannelPointsCustomRewardRedemptionAdd)
        }
        "channel.channel_points_custom_reward_redemption.update" => {
            serde_json::from_value::<ChannelPointsCustomRewardRedemption>(event_json.clone())
                .ok()
                .map(TwitchEventSubData::ChannelPointsCustomRewardRedemptionUpdate)
        }
        _ => None,
    }
}
