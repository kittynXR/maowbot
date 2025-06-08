//! src/eventbus/mod.rs
//!
//! Provides an in-process event bus that supports guaranteed delivery
//! to multiple subscribers via bounded MPSC queues.

pub mod db_logger;
pub mod db_logger_handle;

use std::sync::Arc;
use tokio::sync::{mpsc, watch, Mutex};
use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};
use maowbot_common::models::platform::Platform;

/// Global event type that various parts of the bot can publish or subscribe to.
/// Extend this enum with whatever events your system needs.
#[derive(Debug, Clone)]
pub enum BotEvent {
    /// Represents a chat message, possibly from Twitch, Discord, or VRChat.
    ChatMessage {
        platform: String,
        channel: String,
        user: String,
        text: String,
        timestamp: DateTime<Utc>,
        metadata: serde_json::Map<String, serde_json::Value>,
    },

    /// Periodic heartbeat event, or anything else you broadcast.
    Tick,

    /// Example system-wide event for debugging or administration.
    SystemMessage(String),

    /// NEW: We add a variant for Twitch EventSub notifications.
    /// This wraps a typed event from the newly introduced TwitchEventSubData enum.
    TwitchEventSub(TwitchEventSubData),
}

/// This is the new type used by BotEvent::TwitchEventSub. Each variant corresponds to one of
/// the supported Twitch EventSub event types. For the actual fields, see the `events.rs` file in
/// the `twitch_eventsub` module.
#[derive(Debug, Clone)]
pub enum TwitchEventSubData {
    StreamOnline(crate::platforms::twitch_eventsub::events::StreamOnline),
    StreamOffline(crate::platforms::twitch_eventsub::events::StreamOffline),
    ChannelBitsUse(crate::platforms::twitch_eventsub::events::ChannelBitsUse),
    ChannelUpdate(crate::platforms::twitch_eventsub::events::ChannelUpdate),
    ChannelFollow(crate::platforms::twitch_eventsub::events::ChannelFollow),
    ChannelAdBreakBegin(crate::platforms::twitch_eventsub::events::ChannelAdBreakBegin),
    ChannelChatNotification(crate::platforms::twitch_eventsub::events::ChannelChatNotification),
    ChannelSharedChatBegin(crate::platforms::twitch_eventsub::events::ChannelSharedChatBegin),
    ChannelSharedChatUpdate(crate::platforms::twitch_eventsub::events::ChannelSharedChatUpdate),
    ChannelSharedChatEnd(crate::platforms::twitch_eventsub::events::ChannelSharedChatEnd),
    ChannelSubscribe(crate::platforms::twitch_eventsub::events::ChannelSubscribe),
    ChannelSubscriptionEnd(crate::platforms::twitch_eventsub::events::ChannelSubscriptionEnd),
    ChannelSubscriptionGift(crate::platforms::twitch_eventsub::events::ChannelSubscriptionGift),
    ChannelSubscriptionMessage(crate::platforms::twitch_eventsub::events::ChannelSubscriptionMessage),
    ChannelCheer(crate::platforms::twitch_eventsub::events::ChannelCheer),
    ChannelRaid(crate::platforms::twitch_eventsub::events::ChannelRaid),
    ChannelBan(crate::platforms::twitch_eventsub::events::ChannelBan),
    ChannelUnban(crate::platforms::twitch_eventsub::events::ChannelUnban),
    ChannelUnbanRequestCreate(crate::platforms::twitch_eventsub::events::ChannelUnbanRequestCreate),
    ChannelUnbanRequestResolve(crate::platforms::twitch_eventsub::events::ChannelUnbanRequestResolve),
    ChannelHypeTrainBegin(crate::platforms::twitch_eventsub::events::ChannelHypeTrainBegin),
    ChannelHypeTrainProgress(crate::platforms::twitch_eventsub::events::ChannelHypeTrainProgress),
    ChannelHypeTrainEnd(crate::platforms::twitch_eventsub::events::ChannelHypeTrainEnd),
    ChannelShoutoutCreate(crate::platforms::twitch_eventsub::events::ChannelShoutoutCreate),
    ChannelShoutoutReceive(crate::platforms::twitch_eventsub::events::ChannelShoutoutReceive),
    ChannelPointsAutomaticRewardRedemptionAddV2(
        crate::platforms::twitch_eventsub::events::ChannelPointsAutomaticRewardRedemptionAddV2
    ),
    ChannelPointsCustomRewardAdd(
        crate::platforms::twitch_eventsub::events::ChannelPointsCustomReward
    ),
    ChannelPointsCustomRewardUpdate(
        crate::platforms::twitch_eventsub::events::ChannelPointsCustomReward
    ),
    ChannelPointsCustomRewardRemove(
        crate::platforms::twitch_eventsub::events::ChannelPointsCustomReward
    ),
    ChannelPointsCustomRewardRedemptionAdd(
        crate::platforms::twitch_eventsub::events::ChannelPointsCustomRewardRedemption
    ),
    ChannelPointsCustomRewardRedemptionUpdate(
        crate::platforms::twitch_eventsub::events::ChannelPointsCustomRewardRedemption
    ),
}

impl BotEvent {
    /// Get the event type as a string
    pub fn event_type(&self) -> String {
        match self {
            BotEvent::ChatMessage { .. } => "chat_message".to_string(),
            BotEvent::Tick => "tick".to_string(),
            BotEvent::SystemMessage(_) => "system_message".to_string(),
            BotEvent::TwitchEventSub(data) => match data {
                TwitchEventSubData::StreamOnline(_) => "stream.online".to_string(),
                TwitchEventSubData::StreamOffline(_) => "stream.offline".to_string(),
                TwitchEventSubData::ChannelBitsUse(_) => "channel.bits_use".to_string(),
                TwitchEventSubData::ChannelUpdate(_) => "channel.update".to_string(),
                TwitchEventSubData::ChannelFollow(_) => "channel.follow".to_string(),
                TwitchEventSubData::ChannelAdBreakBegin(_) => "channel.ad_break.begin".to_string(),
                TwitchEventSubData::ChannelChatNotification(_) => "channel.chat.notification".to_string(),
                TwitchEventSubData::ChannelSharedChatBegin(_) => "channel.shared_chat.begin".to_string(),
                TwitchEventSubData::ChannelSharedChatUpdate(_) => "channel.shared_chat.update".to_string(),
                TwitchEventSubData::ChannelSharedChatEnd(_) => "channel.shared_chat.end".to_string(),
                TwitchEventSubData::ChannelSubscribe(_) => "channel.subscribe".to_string(),
                TwitchEventSubData::ChannelSubscriptionEnd(_) => "channel.subscription.end".to_string(),
                TwitchEventSubData::ChannelSubscriptionGift(_) => "channel.subscription.gift".to_string(),
                TwitchEventSubData::ChannelSubscriptionMessage(_) => "channel.subscription.message".to_string(),
                TwitchEventSubData::ChannelCheer(_) => "channel.cheer".to_string(),
                TwitchEventSubData::ChannelRaid(_) => "channel.raid".to_string(),
                TwitchEventSubData::ChannelBan(_) => "channel.ban".to_string(),
                TwitchEventSubData::ChannelUnban(_) => "channel.unban".to_string(),
                TwitchEventSubData::ChannelUnbanRequestCreate(_) => "channel.unban_request.create".to_string(),
                TwitchEventSubData::ChannelUnbanRequestResolve(_) => "channel.unban_request.resolve".to_string(),
                TwitchEventSubData::ChannelHypeTrainBegin(_) => "channel.hype_train.begin".to_string(),
                TwitchEventSubData::ChannelHypeTrainProgress(_) => "channel.hype_train.progress".to_string(),
                TwitchEventSubData::ChannelHypeTrainEnd(_) => "channel.hype_train.end".to_string(),
                TwitchEventSubData::ChannelShoutoutCreate(_) => "channel.shoutout.create".to_string(),
                TwitchEventSubData::ChannelShoutoutReceive(_) => "channel.shoutout.receive".to_string(),
                TwitchEventSubData::ChannelPointsAutomaticRewardRedemptionAddV2(_) => "channel.channel_points_automatic_reward_redemption.add".to_string(),
                TwitchEventSubData::ChannelPointsCustomRewardAdd(_) => "channel.channel_points_custom_reward.add".to_string(),
                TwitchEventSubData::ChannelPointsCustomRewardUpdate(_) => "channel.channel_points_custom_reward.update".to_string(),
                TwitchEventSubData::ChannelPointsCustomRewardRemove(_) => "channel.channel_points_custom_reward.remove".to_string(),
                TwitchEventSubData::ChannelPointsCustomRewardRedemptionAdd(_) => "channel.channel_points_custom_reward_redemption.add".to_string(),
                TwitchEventSubData::ChannelPointsCustomRewardRedemptionUpdate(_) => "channel.channel_points_custom_reward_redemption.update".to_string(),
            }
        }
    }
    
    /// Get the platform for this event
    pub fn platform(&self) -> Option<Platform> {
        match self {
            BotEvent::ChatMessage { platform, .. } => Some(Platform::from_string(platform)),
            BotEvent::TwitchEventSub(_) => Some(Platform::TwitchEventSub),
            _ => None,
        }
    }
}

/// Each subscriber gets its own `mpsc::Sender<BotEvent>` for guaranteed delivery.
///
/// - If the subscriber’s channel buffer fills, `publish` will await
///   until there's space (backpressure).
/// - If the subscriber has dropped the `Receiver`, the channel is closed
///   and sending returns an error.
#[derive(Clone)]
pub struct EventBus {
    subscribers: Arc<Mutex<Vec<mpsc::Sender<BotEvent>>>>,
    shutdown_tx: watch::Sender<bool>,
    pub shutdown_rx: watch::Receiver<bool>,
}

/// Default size for each subscriber’s buffer. Adjust as needed.
const DEFAULT_BUFFER_SIZE: usize = 10000;

impl EventBus {
    /// Create a new, empty event bus.
    pub fn new() -> Self {
        let (tx, rx) = watch::channel(false);
        Self {
            subscribers: Arc::new(Mutex::new(vec![])),
            shutdown_tx: tx,
            shutdown_rx: rx,
        }
    }

    pub fn shutdown(&self) {
        // Setting watch to true
        let _ = self.shutdown_tx.send(true);
    }

    pub fn is_shutdown(&self) -> bool {
        *self.shutdown_rx.borrow()
    }

    /// Returns a receiver on which events will be delivered.
    pub async fn subscribe(&self, buffer_size: Option<usize>) -> mpsc::Receiver<BotEvent> {
        let size = buffer_size.unwrap_or(DEFAULT_BUFFER_SIZE);
        let (tx, rx) = mpsc::channel(size);
        let mut subs = self.subscribers.lock().await;
        subs.push(tx);
        rx
    }

    /// Publish an event to all subscribers.
    pub async fn publish(&self, event: BotEvent) {
        let senders = {
            let subs = self.subscribers.lock().await;
            subs.clone()
        };
        for s in senders {
            let _ = s.send(event.clone()).await;
        }
    }

    /// Convenience method: publish a `ChatMessage` event.
    pub async fn publish_chat(
        &self,
        platform: &str,
        channel: &str,
        user: &str,
        text: &str,
    ) {
        let event = BotEvent::ChatMessage {
            platform: platform.to_string(),
            channel: channel.to_string(),
            user: user.to_string(),
            text: text.to_string(),
            timestamp: Utc::now(),
            metadata: serde_json::Map::new(),
        };
        self.publish(event).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{sleep, timeout, Duration};

    #[tokio::test]
    async fn test_subscribers_receive_events() {
        let bus = EventBus::new();

        let mut rx1 = bus.subscribe(Some(5)).await;
        let mut rx2 = bus.subscribe(Some(5)).await;

        // Publish an event
        bus.publish(BotEvent::Tick).await;

        // Both subscribers should get it
        let evt1 = rx1.recv().await.expect("rx1 should get event");
        let evt2 = rx2.recv().await.expect("rx2 should get event");

        match evt1 {
            BotEvent::Tick => { /* OK */ }
            _ => panic!("rx1 got the wrong event type"),
        }
        match evt2 {
            BotEvent::Tick => { /* OK */ }
            _ => panic!("rx2 got the wrong event type"),
        }
    }

    #[tokio::test]
    async fn test_backpressure_blocking() {
        let bus = EventBus::new();
        let mut rx = bus.subscribe(Some(1)).await; // queue size = 1

        // Publish first message to fill the queue.
        bus.publish(BotEvent::SystemMessage("msg1".into())).await;

        // Spawn a task that reads the two messages after a short delay.
        let handle = tokio::spawn(async move {
            sleep(Duration::from_millis(50)).await;
            let first = rx.recv().await.expect("expected first message");
            let second = rx.recv().await.expect("expected second message");
            (first, second)
        });

        // Publish the second message (this call will wait until there's space).
        let second_publish = bus.publish(BotEvent::SystemMessage("msg2".into()));
        let result = timeout(Duration::from_millis(500), second_publish).await;
        assert!(result.is_ok(), "publish should eventually unblock");

        let (evt1, evt2) = handle.await.unwrap();
        if let BotEvent::SystemMessage(txt) = evt1 {
            assert_eq!(txt, "msg1");
        } else {
            panic!("first message mismatch");
        }
        if let BotEvent::SystemMessage(txt) = evt2 {
            assert_eq!(txt, "msg2");
        } else {
            panic!("second message mismatch");
        }
    }

    #[tokio::test]
    async fn test_no_drop_when_queue_is_full() {
        let bus = EventBus::new();
        let mut rx = bus.subscribe(Some(1)).await;

        // Fill the queue.
        bus.publish(BotEvent::SystemMessage("first".into())).await;

        // Spawn a task that sleeps and then reads both messages.
        let handle = tokio::spawn(async move {
            sleep(Duration::from_millis(50)).await;
            let first_evt = rx.recv().await.unwrap();
            let second_evt = rx.recv().await.unwrap();
            (first_evt, second_evt)
        });

        // Attempt to publish the second message (must wait until the subscriber reads).
        let publish_fut = bus.publish(BotEvent::SystemMessage("second".into()));
        let publish_res = timeout(Duration::from_millis(300), publish_fut).await;
        assert!(publish_res.is_ok(), "publish should eventually succeed");

        // Check the received messages.
        let (evt1, evt2) = handle.await.unwrap();
        if let BotEvent::SystemMessage(txt) = evt1 {
            assert_eq!(txt, "first");
        } else {
            panic!("First message mismatch");
        }
        if let BotEvent::SystemMessage(txt) = evt2 {
            assert_eq!(txt, "second");
        } else {
            panic!("Second message mismatch");
        }
    }
}
