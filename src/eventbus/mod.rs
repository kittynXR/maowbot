//! src/eventbus/mod.rs
//!
//! Provides an in-process event bus that supports guaranteed delivery
//! to multiple subscribers via bounded MPSC queues.

pub mod db_logger;

use std::sync::{Arc, Mutex};
use tokio::sync::{mpsc, watch};
use chrono::{DateTime, Utc};

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
    },

    /// Periodic heartbeat event, or anything else you broadcast.
    Tick,

    /// Example system-wide event for debugging or administration.
    SystemMessage(String),
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
    pub(crate) shutdown_rx: watch::Receiver<bool>,
}

/// Default size for each subscriber’s buffer. Adjust as needed.
const DEFAULT_BUFFER_SIZE: usize = 200;

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

    /// Create a new subscriber with a bounded buffer size (or use `DEFAULT_BUFFER_SIZE`).
    /// Returns a Receiver that the subscriber can poll for events.
    pub fn subscribe(&self, buffer_size: Option<usize>) -> mpsc::Receiver<BotEvent> {
        let size = buffer_size.unwrap_or(DEFAULT_BUFFER_SIZE);
        let (tx, rx) = mpsc::channel(size);

        let mut subs = self.subscribers.lock().unwrap();
        subs.push(tx);

        rx
    }

    /// Publish an event to all subscribers. If any subscriber’s buffer is full, this call
    /// will `.await` until space is free, ensuring no messages are lost.
    pub async fn publish(&self, event: BotEvent) {
        // Clone the senders outside the lock
        let senders = {
            let guard = self.subscribers.lock().unwrap();
            guard.clone()
        };
        // Now send the event to each subscriber
        for s in senders {
            let _ = s.send(event.clone()).await;
        }
    }

    /// Optional convenience method: publish a `ChatMessage` event with minimal boilerplate.
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
        };
        self.publish(event).await;
    }
}

/// Example unit tests for the event bus itself.
#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{timeout, Duration};
    use tokio::time::sleep;

    /// Basic test: multiple subscribers each receive the same event.
    #[tokio::test]
    async fn test_subscribers_receive_events() {
        let bus = EventBus::new();

        let mut rx1 = bus.subscribe(Some(5));
        let mut rx2 = bus.subscribe(Some(5));

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

    /// Demonstrates concurrency-based backpressure:
    /// We fill a 1-slot queue, then publish a second event, which must block
    /// until the subscriber reads from the channel.
    ///
    /// We use a multi-thread flavor so the blocking publish and the subscriber
    /// can run in parallel.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_backpressure_blocking() {
        let bus = EventBus::new();
        let mut rx = bus.subscribe(Some(1)); // queue size=1

        // 1) Publish first message => fills the queue
        bus.publish(BotEvent::SystemMessage("msg1".into())).await;

        // 2) Spawn a separate task that will read them after a short delay.
        let handle = tokio::spawn(async move {
            // Sleep to ensure the second publish is truly blocked until we read
            sleep(Duration::from_millis(50)).await;
            let first = rx.recv().await.expect("expected first message");
            let second = rx.recv().await.expect("expected second message");
            (first, second)
        });

        // 3) Attempt the second publish => should block until there's space
        let second_publish = bus.publish(BotEvent::SystemMessage("msg2".into()));

        // Use a timeout to ensure we don't hang forever. We'll wait 500ms.
        let result = timeout(Duration::from_millis(500), second_publish).await;
        assert!(result.is_ok(), "publish should eventually unblock after the subscriber reads");

        // 4) Confirm the subscriber eventually read both
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

    /// This simpler test ensures that if the queue is full, it waits for a read
    /// rather than dropping the message. We do partial reads in the same task.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_no_drop_when_queue_is_full() {
        let bus = EventBus::new();
        let mut rx = bus.subscribe(Some(1));

        // 1) Fill the queue with "first"
        bus.publish(BotEvent::SystemMessage("first".into())).await;

        // 2) Spawn a reading task that sleeps, then reads both messages
        let handle = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(50)).await;
            let first_evt = rx.recv().await.unwrap();
            let second_evt = rx.recv().await.unwrap();
            (first_evt, second_evt)
        });

        // 3) Attempt the second publish => must wait until the subscriber reads
        let publish_fut = bus.publish(BotEvent::SystemMessage("second".into()));
        let publish_res = tokio::time::timeout(std::time::Duration::from_millis(300), publish_fut).await;
        assert!(publish_res.is_ok(), "publish should eventually succeed (after we read)");

        // 4) Check the results
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
