//! tests/eventbus_tests.rs
use std::sync::{Arc, Mutex};
use async_trait::async_trait;
use chrono::Utc;
use tokio::time::Duration;

// Our crate modules
use maowbot::Error;
use maowbot::eventbus::{EventBus, BotEvent};
use maowbot::eventbus::db_logger::spawn_db_logger_task;
use maowbot::plugins::manager::{PluginManager, PluginConnection, PluginConnectionInfo};
// For the analytics trait and data
use maowbot::repositories::sqlite::analytics::{ChatMessage, AnalyticsRepo};

// ---------- Mock Analytics Repo ----------
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
        let mut lock = self.messages.lock().unwrap();
        lock.push(msg.clone());
        Ok(())
    }
}

// ---------- Mock Plugin ----------
#[derive(Clone)]
struct MockPlugin {
    info: Arc<Mutex<PluginConnectionInfo>>,
    received: Arc<Mutex<Vec<String>>>,
}

#[async_trait]
impl PluginConnection for MockPlugin {
    fn info(&self) -> PluginConnectionInfo {
        self.info.lock().unwrap().clone()
    }

    fn set_capabilities(&self, caps: Vec<maowbot::plugins::capabilities::PluginCapability>) {
        self.info.lock().unwrap().capabilities = caps;
    }

    fn send(&self, event: maowbot::plugins::protocol::BotToPlugin) -> Result<(), Error> {
        let mut lock = self.received.lock().unwrap();
        lock.push(format!("{:?}", event));
        Ok(())
    }

    async fn stop(&self) -> Result<(), Error> {
        Ok(())
    }

    // ADD the as_any method so we can downcast in the test
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

// ---------- The actual test ----------
#[tokio::test]
async fn test_eventbus_integration() {
    // 1) Create an EventBus
    let bus = Arc::new(EventBus::new());

    // 2) Create a MockAnalyticsRepo
    let mock_repo = MockAnalyticsRepo::new();

    // 3) Spawn the DB logger task with short flush interval
    spawn_db_logger_task(&bus, mock_repo.clone(), 10, 1);

    // 4) Create a PluginManager, subscribe to bus
    let plugin_mgr = PluginManager::new(None);
    plugin_mgr.subscribe_to_event_bus(bus.clone());
    // tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // 5) Add a mock plugin
    let mock_plugin = MockPlugin {
        info: Arc::new(Mutex::new(PluginConnectionInfo {
            name: "mock_plugin".into(),
            capabilities: vec![],
        })),
        received: Arc::new(Mutex::new(vec![])),
    };
    {
        let mut list = plugin_mgr.plugin_list();
        list.push(Arc::new(mock_plugin.clone()) as Arc<dyn PluginConnection>);
    }

    // 6) Publish a single ChatMessage
    let evt = BotEvent::ChatMessage {
        platform: "test_platform".into(),
        channel: "test_channel".into(),
        user: "test_user".into(),
        text: "hello world".into(),
        timestamp: Utc::now(),
    };
    bus.publish(evt).await;

    // 7) Sleep a bit to ensure it's queued
    tokio::time::sleep(Duration::from_millis(50)).await;

    // 8) Trigger a final flush
    bus.shutdown();
    tokio::time::sleep(Duration::from_millis(50)).await;

    // 9) Check the DB
    let lock = mock_repo.messages.lock().unwrap();
    assert_eq!(lock.len(), 1, "DB logger should have inserted 1 message");
    assert_eq!(lock[0].message_text, "hello world");
    drop(lock);

    // 10) Check the plugin
    let plugins = plugin_mgr.plugin_list();
    let plugin_dyn = plugins[0].clone();
    // now downcast
    let maybe_mock = plugin_dyn.as_any().downcast_ref::<MockPlugin>();
    assert!(maybe_mock.is_some(), "Should be our MockPlugin");
    let mock_ref = maybe_mock.unwrap();
    let recvd = mock_ref.received.lock().unwrap();
    assert_eq!(recvd.len(), 1, "Should have 1 inbound event");
    assert!(recvd[0].contains("ChatMessage"));

    // done
}
