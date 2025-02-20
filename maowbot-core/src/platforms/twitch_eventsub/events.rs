// File: maowbot-core/src/platforms/twitch_eventsub/events.rs

use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use uuid::Uuid;

/// Each subscription wrapper has metadata like `id`, `type`, etc.
/// We'll store it in a generic struct:
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SubscriptionData {
    pub id: String,
    #[serde(rename = "type")]
    pub sub_type: String,
    pub version: String,
    pub status: String,
    pub cost: u32,

    #[serde(default)]
    pub condition: serde_json::Value,

    #[serde(default)]
    pub transport: serde_json::Value,

    #[serde(default)]
    pub created_at: Option<DateTime<Utc>>,
}

/// The top-level wrapper from Twitch for a "notification" message is:
/// { "subscription": { ... }, "event": { ... } }
#[derive(Debug, Clone, Deserialize)]
pub struct EventSubNotificationEnvelope {
    pub subscription: SubscriptionData,
    pub event: serde_json::Value,
}

// --------------------------------------------------------------------------------
// For each subscription type, define the structured data for `event`
// --------------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
pub struct ChannelBitsUse {
    pub user_id: String,
    pub user_login: String,
    pub user_name: String,
    pub broadcaster_user_id: String,
    pub broadcaster_user_login: String,
    pub broadcaster_user_name: String,
    pub bits: u64,
    #[serde(rename = "type")]
    pub usage_type: String, // e.g. "cheer", "power_up"
    pub power_up: Option<serde_json::Value>,
    pub message: BitsMessage,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BitsMessage {
    pub text: String,
    pub fragments: Vec<BitsFragment>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BitsFragment {
    #[serde(rename = "type")]
    pub fragment_type: String,
    pub text: String,
    #[serde(default)]
    pub cheermote: Option<serde_json::Value>,
    #[serde(default)]
    pub emote: Option<serde_json::Value>,
}

// ------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
pub struct ChannelUpdate {
    pub broadcaster_user_id: String,
    pub broadcaster_user_login: String,
    pub broadcaster_user_name: String,
    pub title: String,
    pub language: String,
    pub category_id: String,
    pub category_name: String,
    #[serde(default)]
    pub content_classification_labels: Vec<String>,
}

// ------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
pub struct ChannelFollow {
    pub user_id: String,
    pub user_login: String,
    pub user_name: String,
    pub broadcaster_user_id: String,
    pub broadcaster_user_login: String,
    pub broadcaster_user_name: String,
    pub followed_at: DateTime<Utc>,
}

// ------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
pub struct ChannelAdBreakBegin {
    pub duration_seconds: String,
    pub started_at: DateTime<Utc>,
    pub is_automatic: String,
    pub broadcaster_user_id: String,
    pub broadcaster_user_login: String,
    pub broadcaster_user_name: String,
    pub requester_user_id: String,
    pub requester_user_login: String,
    pub requester_user_name: String,
}

// ------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
pub struct ChannelChatNotification {
    pub broadcaster_user_id: String,
    pub broadcaster_user_login: String,
    pub broadcaster_user_name: String,
    pub chatter_user_id: String,
    pub chatter_user_login: String,
    pub chatter_user_name: String,
    pub chatter_is_anonymous: bool,
    pub color: String,
    #[serde(default)]
    pub badges: Vec<serde_json::Value>,
    pub system_message: String,
    pub message_id: String,
    pub message: ChatMessageBody,
    #[serde(default)]
    pub notice_type: String,
    // The many fields that might be present:
    #[serde(default)]
    pub sub: Option<serde_json::Value>,
    #[serde(default)]
    pub resub: Option<serde_json::Value>,
    #[serde(default)]
    pub sub_gift: Option<serde_json::Value>,
    #[serde(default)]
    pub community_sub_gift: Option<serde_json::Value>,
    #[serde(default)]
    pub gift_paid_upgrade: Option<serde_json::Value>,
    #[serde(default)]
    pub prime_paid_upgrade: Option<serde_json::Value>,
    #[serde(default)]
    pub pay_it_forward: Option<serde_json::Value>,
    #[serde(default)]
    pub raid: Option<serde_json::Value>,
    #[serde(default)]
    pub unraid: Option<serde_json::Value>,
    #[serde(default)]
    pub announcement: Option<serde_json::Value>,
    #[serde(default)]
    pub bits_badge_tier: Option<serde_json::Value>,
    #[serde(default)]
    pub charity_donation: Option<serde_json::Value>,
    #[serde(default)]
    pub shared_chat_sub: Option<serde_json::Value>,
    #[serde(default)]
    pub shared_chat_resub: Option<serde_json::Value>,
    #[serde(default)]
    pub shared_chat_sub_gift: Option<serde_json::Value>,
    #[serde(default)]
    pub shared_chat_community_sub_gift: Option<serde_json::Value>,
    #[serde(default)]
    pub shared_chat_gift_paid_upgrade: Option<serde_json::Value>,
    #[serde(default)]
    pub shared_chat_prime_paid_upgrade: Option<serde_json::Value>,
    #[serde(default)]
    pub shared_chat_pay_it_forward: Option<serde_json::Value>,
    #[serde(default)]
    pub shared_chat_raid: Option<serde_json::Value>,
    #[serde(default)]
    pub shared_chat_unraid: Option<serde_json::Value>,
    #[serde(default)]
    pub shared_chat_announcement: Option<serde_json::Value>,
    #[serde(default)]
    pub shared_chat_bits_badge_tier: Option<serde_json::Value>,
    #[serde(default)]
    pub shared_chat_charity_donation: Option<serde_json::Value>,
    #[serde(default)]
    pub source_broadcaster_user_id: Option<String>,
    #[serde(default)]
    pub source_broadcaster_user_login: Option<String>,
    #[serde(default)]
    pub source_broadcaster_user_name: Option<String>,
    #[serde(default)]
    pub source_message_id: Option<String>,
    #[serde(default)]
    pub source_badges: Option<Vec<serde_json::Value>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ChatMessageBody {
    pub text: String,
    #[serde(default)]
    pub fragments: Vec<serde_json::Value>,
}

// ------------------------------------------------------------------------

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
pub struct Participant {
    pub broadcaster_user_id: String,
    pub broadcaster_user_login: String,
    pub broadcaster_user_name: String,
}

// ------------------------------------------------------------------------

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

// ------------------------------------------------------------------------

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

// ------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
pub struct ChannelSubscribe {
    pub user_id: String,
    pub user_login: String,
    pub user_name: String,
    pub broadcaster_user_id: String,
    pub broadcaster_user_login: String,
    pub broadcaster_user_name: String,
    pub tier: String,
    pub is_gift: bool,
}

// ------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
pub struct ChannelSubscriptionEnd {
    pub user_id: String,
    pub user_login: String,
    pub user_name: String,
    pub broadcaster_user_id: String,
    pub broadcaster_user_login: String,
    pub broadcaster_user_name: String,
    pub tier: String,
    pub is_gift: bool,
}

// ------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
pub struct ChannelSubscriptionGift {
    pub user_id: String,
    pub user_login: String,
    pub user_name: String,
    pub broadcaster_user_id: String,
    pub broadcaster_user_login: String,
    pub broadcaster_user_name: String,
    pub total: u64,
    pub tier: String,
    pub cumulative_total: Option<u64>,
    pub is_anonymous: bool,
}

// ------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
pub struct ChannelSubscriptionMessage {
    pub user_id: String,
    pub user_login: String,
    pub user_name: String,
    pub broadcaster_user_id: String,
    pub broadcaster_user_login: String,
    pub broadcaster_user_name: String,
    pub tier: String,
    pub message: SubMessage,
    pub cumulative_months: u32,
    pub streak_months: Option<u32>,
    pub duration_months: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SubMessage {
    pub text: String,
    #[serde(default)]
    pub emotes: Vec<SubEmote>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SubEmote {
    pub begin: u32,
    pub end: u32,
    pub id: String,
}

// ------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
pub struct ChannelCheer {
    pub is_anonymous: bool,
    pub user_id: Option<String>,
    pub user_login: Option<String>,
    pub user_name: Option<String>,
    pub broadcaster_user_id: String,
    pub broadcaster_user_login: String,
    pub broadcaster_user_name: String,
    pub message: String,
    pub bits: u64,
}

// ------------------------------------------------------------------------

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

// ------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
pub struct ChannelBan {
    pub user_id: String,
    pub user_login: String,
    pub user_name: String,
    pub broadcaster_user_id: String,
    pub broadcaster_user_login: String,
    pub broadcaster_user_name: String,
    pub moderator_user_id: String,
    pub moderator_user_login: String,
    pub moderator_user_name: String,
    pub reason: String,
    pub banned_at: DateTime<Utc>,
    pub ends_at: Option<DateTime<Utc>>,
    pub is_permanent: bool,
}

// ------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
pub struct ChannelUnban {
    pub user_id: String,
    pub user_login: String,
    pub user_name: String,
    pub broadcaster_user_id: String,
    pub broadcaster_user_login: String,
    pub broadcaster_user_name: String,
    pub moderator_user_id: String,
    pub moderator_user_login: String,
    pub moderator_user_name: String,
}

// ------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
pub struct ChannelUnbanRequestCreate {
    pub id: String,
    pub broadcaster_user_id: String,
    pub broadcaster_user_login: String,
    pub broadcaster_user_name: String,
    pub user_id: String,
    pub user_login: String,
    pub user_name: String,
    pub text: String,
    pub created_at: DateTime<Utc>,
}

// ------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
pub struct ChannelUnbanRequestResolve {
    pub id: String,
    pub broadcaster_user_id: String,
    pub broadcaster_user_login: String,
    pub broadcaster_user_name: String,
    pub moderator_user_id: String,
    pub moderator_user_login: String,
    pub moderator_user_name: String,
    pub user_id: String,
    pub user_login: String,
    pub user_name: String,
    pub resolution_text: String,
    pub status: String,
}

// ------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
pub struct ChannelHypeTrainBegin {
    pub id: String,
    pub broadcaster_user_id: String,
    pub broadcaster_user_login: String,
    pub broadcaster_user_name: String,
    pub total: u64,
    pub progress: u64,
    pub goal: u64,
    #[serde(default)]
    pub top_contributions: Vec<Contribution>,
    pub last_contribution: Contribution,
    pub level: u32,
    pub started_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub is_golden_kappa_train: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Contribution {
    pub user_id: String,
    pub user_login: String,
    pub user_name: String,
    #[serde(rename = "type")]
    pub ctype: String,
    pub total: u64,
}

// ------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
pub struct ChannelHypeTrainProgress {
    pub id: String,
    pub broadcaster_user_id: String,
    pub broadcaster_user_login: String,
    pub broadcaster_user_name: String,
    pub level: u32,
    pub total: u64,
    pub progress: u64,
    pub goal: u64,
    #[serde(default)]
    pub top_contributions: Vec<Contribution>,
    pub last_contribution: Contribution,
    pub started_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub is_golden_kappa_train: bool,
}

// ------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
pub struct ChannelHypeTrainEnd {
    pub id: String,
    pub broadcaster_user_id: String,
    pub broadcaster_user_login: String,
    pub broadcaster_user_name: String,
    pub level: u32,
    pub total: u64,
    #[serde(default)]
    pub top_contributions: Vec<Contribution>,
    pub started_at: DateTime<Utc>,
    pub ended_at: DateTime<Utc>,
    pub cooldown_ends_at: DateTime<Utc>,
    pub is_golden_kappa_train: bool,
}

// ------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
pub struct ChannelShoutoutCreate {
    pub broadcaster_user_id: String,
    pub broadcaster_user_name: String,
    pub broadcaster_user_login: String,
    pub moderator_user_id: String,
    pub moderator_user_name: String,
    pub moderator_user_login: String,
    pub to_broadcaster_user_id: String,
    pub to_broadcaster_user_name: String,
    pub to_broadcaster_user_login: String,
    pub started_at: DateTime<Utc>,
    pub viewer_count: u64,
    pub cooldown_ends_at: DateTime<Utc>,
    pub target_cooldown_ends_at: DateTime<Utc>,
}

// ------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
pub struct ChannelShoutoutReceive {
    pub broadcaster_user_id: String,
    pub broadcaster_user_name: String,
    pub broadcaster_user_login: String,
    pub from_broadcaster_user_id: String,
    pub from_broadcaster_user_name: String,
    pub from_broadcaster_user_login: String,
    pub viewer_count: u64,
    pub started_at: DateTime<Utc>,
}

// ------------------------------------------------------------------------
// Helper function to parse from JSON into a known event type. Returns None if unknown.
// ------------------------------------------------------------------------

use crate::eventbus::TwitchEventSubData;

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
        _ => None,
    }
}
