// tests/eventbus_tests.rs

use std::sync::Arc;
use async_trait::async_trait;
use chrono::Utc;
use tokio::time::Duration;
use tokio::sync::Mutex;

use maowbot::Error;
use maowbot::eventbus::{EventBus, BotEvent};
use maowbot::eventbus::db_logger::spawn_db_logger_task;
use maowbot::plugins::manager::{
    PluginManager, PluginConnection, PluginConnectionInfo
};
use maowbot::plugins::proto::plugs::{
    PluginCapability,
    PluginStreamResponse,
};
use maowbot::repositories::sqlite::analytics::{ChatMessage, AnalyticsRepo};

/// ---------- Mock Analytics Repo ----------
#[derive(Clone)]
struct MockAnalyticsRepo {
    messages: Arc<Mutex<Vec<ChatMessage>>>,
}

impl MockAnalyticsRepo {
    fn new() -> Self {
        Self {
            messages: Arc::new(Mutex::new(vec![])),
        }
    }
}

#[async_trait]
impl AnalyticsRepo for MockAnalyticsRepo {
    async fn insert_chat_message(&self, msg: &ChatMessage) -> Result<(), Error> {
        let mut lock = self.messages.lock().await;
        lock.push(msg.clone());
        Ok(())
    }
}

/// ---------- Mock Plugin ----------
#[derive(Clone)]
struct MockPlugin {
    info: Arc<Mutex<PluginConnectionInfo>>,
    received: Arc<Mutex<Vec<String>>>,
}

impl MockPlugin {
    fn new(name: &str, capabilities: Vec<PluginCapability>) -> Self {
        Self {
            info: Arc::new(Mutex::new(PluginConnectionInfo {
                name: name.into(),
                capabilities,
            })),
            received: Arc::new(Mutex::new(vec![])),
        }
    }
}

#[async_trait]
impl PluginConnection for MockPlugin {
    async fn info(&self) -> PluginConnectionInfo {
        let guard = self.info.lock().await;
        guard.clone()
    }

    async fn set_capabilities(&self, caps: Vec<PluginCapability>) {
        let mut guard = self.info.lock().await;
        guard.capabilities = caps;
    }

    /// Replaces the old `send(&self, event: BotToPlugin)` signature
    async fn send(&self, response: PluginStreamResponse) -> Result<(), Error> {
        // Store the debug-printed form of the response
        let mut lock = self.received.lock().await;
        lock.push(format!("{:?}", response));
        Ok(())
    }

    async fn stop(&self) -> Result<(), Error> {
        Ok(())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// ---------- The actual test ----------
#[tokio::test]
async fn test_eventbus_integration() -> Result<(), Error> {
    // 1) Create an EventBus
    let bus = Arc::new(EventBus::new());

    // 2) Create a MockAnalyticsRepo
    let mock_repo = MockAnalyticsRepo::new();

    // 3) Spawn the DB logger task with short flush interval
    spawn_db_logger_task(&bus, mock_repo.clone(), 10, 1);

    // 4) Create a PluginManager, subscribe to the bus
    let plugin_mgr = PluginManager::new(None);
    plugin_mgr.subscribe_to_event_bus(bus.clone());

    // 5) Add a mock plugin that can receive chat events
    let mock_plugin = MockPlugin::new("mock_plugin", vec![PluginCapability::ReceiveChatEvents]);
    {
        let mut list = plugin_mgr.plugins.lock().await;
        list.push(Arc::new(mock_plugin.clone()) as Arc<dyn PluginConnection>);
    }

    // 6) Publish a single ChatMessage event
    let evt = BotEvent::ChatMessage {
        platform: "test_platform".into(),
        channel: "test_channel".into(),
        user: "test_user".into(),
        text: "hello world".into(),
        timestamp: Utc::now(),
    };
    bus.publish(evt).await;

    // 7) Sleep briefly to ensure it’s logged/batched
    tokio::time::sleep(Duration::from_millis(50)).await;

    // 8) Trigger a final flush
    bus.shutdown();
    tokio::time::sleep(Duration::from_millis(50)).await;

    // 9) Check that the DB logger inserted the message
    let lock = mock_repo.messages.lock().await;
    assert_eq!(lock.len(), 1, "DB logger should have inserted 1 message");
    assert_eq!(lock[0].message_text, "hello world");

    // 10) Check the plugin’s received events
    let recvd = mock_plugin.received.lock().await;
    assert_eq!(recvd.len(), 1, "Should have 1 inbound event");
    assert!(
        recvd[0].contains("ChatMessage"),
        "Expected debug output containing ChatMessage"
    );

    Ok(())
}
