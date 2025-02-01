// tests/shutdown_tests.rs

use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::{sleep, Duration};
use chrono::Utc;
use anyhow::Result;

use maowbot::{Database, Error};
use maowbot::eventbus::{EventBus, BotEvent};
use maowbot::eventbus::db_logger::spawn_db_logger_task;
use maowbot::plugins::manager::{
    PluginManager, PluginConnection, PluginConnectionInfo
};
use maowbot_proto::plugs::{
    PluginCapability,
    PluginStreamResponse,
    PluginStreamRequest,
    plugin_stream_request::Payload as ReqPayload,
    plugin_stream_response::Payload as RespPayload,
    Shutdown,
};
use maowbot::repositories::sqlite::analytics::{SqliteAnalyticsRepository, AnalyticsRepo};

use async_trait::async_trait;
use sqlx::Executor;

/// A trivial mock plugin that can (in principle) send a Shutdown message to the bot manager.
#[derive(Clone)]
struct ShutdownTestPlugin {
    info: Arc<Mutex<PluginConnectionInfo>>,
    // Collect events we receive from bot->plugin
    received_responses: Arc<Mutex<Vec<String>>>,
}

impl ShutdownTestPlugin {
    fn new(name: &str, capabilities: Vec<PluginCapability>) -> Self {
        Self {
            info: Arc::new(Mutex::new(PluginConnectionInfo {
                name: name.to_string(),
                capabilities,
            })),
            received_responses: Arc::new(Mutex::new(vec![])),
        }
    }
}

#[async_trait]
impl PluginConnection for ShutdownTestPlugin {
    async fn info(&self) -> PluginConnectionInfo {
        self.info.lock().await.clone()
    }

    async fn set_capabilities(&self, caps: Vec<PluginCapability>) {
        let mut guard = self.info.lock().await;
        guard.capabilities = caps;
    }

    async fn set_name(&self, new_name: String) {
        let mut guard = self.info.lock().await;
        guard.name = new_name;
    }

    async fn send(&self, response: PluginStreamResponse) -> std::result::Result<(), Error> {
        // We just store debug text for later inspection
        let mut lock = self.received_responses.lock().await;
        lock.push(format!("{:?}", response));
        Ok(())
    }

    async fn stop(&self) -> std::result::Result<(), Error> {
        Ok(())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// Test that calling `event_bus.shutdown()` flushes any queued ChatMessages.
#[tokio::test]
async fn test_graceful_shutdown_data_flush() -> Result<()> {
    let db = Database::new(":memory:").await?;
    db.migrate().await?;

    // Insert some dummy users for foreign key references via a transaction
    {
        let mut tx = db.pool().begin().await?;
        for i in 0..3 {
            let user_id = format!("user_{i}");
            tx.execute(
                sqlx::query(
                    r#"INSERT INTO users (user_id, created_at, last_seen, is_active)
                       VALUES (?, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP, 1)"#
                )
                    .bind(user_id)
            ).await?;
        }
        tx.commit().await?;
    }

    let analytics_repo = SqliteAnalyticsRepository::new(db.pool().clone());
    let event_bus = Arc::new(EventBus::new());

    // logger
    let logger_handle = spawn_db_logger_task(&event_bus, analytics_repo.clone(), 5, 1);

    // Publish some chat messages
    for i in 0..3 {
        event_bus
            .publish(BotEvent::ChatMessage {
                platform: "shutdown_test".to_string(),
                channel: "my_channel".to_string(),
                user: format!("user_{i}"),
                text: format!("message number {i}"),
                timestamp: Utc::now(),
            })
            .await;
    }

    tokio::time::sleep(Duration::from_millis(50)).await;

    // Graceful shutdown
    event_bus.shutdown();

    // Wait for logger to flush
    logger_handle.await.unwrap();

    // Confirm those 3 messages got inserted
    let rows = analytics_repo
        .get_recent_messages("shutdown_test", "my_channel", 10)
        .await?;
    assert_eq!(
        rows.len(),
        3,
        "All 3 chat messages should have been flushed"
    );

    Ok(())
}

/// Test that if a plugin sends `Shutdown` request (plugin->bot), the manager calls event_bus.shutdown().
#[tokio::test]
async fn test_plugin_initiated_shutdown() -> Result<()> {
    let db = Database::new(":memory:").await?;
    db.migrate().await?;

    // Insert a user row for foreign key usage
    {
        let mut tx = db.pool().begin().await?;
        tx.execute(
            sqlx::query(
                r#"INSERT INTO users (user_id, created_at, last_seen, is_active)
                   VALUES ('some_user', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP, 1)"#
            )
        ).await?;
        tx.commit().await?;
    }

    // Create repo + event bus + logger
    let analytics_repo = SqliteAnalyticsRepository::new(db.pool().clone());
    let event_bus = Arc::new(EventBus::new());
    let logger_handle = spawn_db_logger_task(&event_bus, analytics_repo.clone(), 5, 1);

    // Create plugin manager
    let mut plugin_mgr = PluginManager::new(None);
    plugin_mgr.set_event_bus(event_bus.clone());
    plugin_mgr.subscribe_to_event_bus(event_bus.clone());

    // Add plugin
    let plugin = ShutdownTestPlugin::new("shutdown_initiator", vec![]);
    {
        let mut lock = plugin_mgr.plugins.lock().await;
        lock.push(Arc::new(plugin.clone()));
    }

    // Publish a chat message first
    event_bus
        .publish(BotEvent::ChatMessage {
            platform: "plugin_shutdown_test".to_string(),
            channel: "chan".to_string(),
            user: "some_user".to_string(),
            text: "hello from user".to_string(),
            timestamp: Utc::now(),
        })
        .await;

    tokio::time::sleep(Duration::from_millis(50)).await;

    // --- Emulate plugin => manager "Shutdown" message:
    plugin_mgr
        .on_inbound_message(
            ReqPayload::Shutdown(Shutdown {}),
            Arc::new(plugin.clone())
        )
        .await;

    // Now the event bus should be shut down
    assert!(event_bus.is_shutdown(), "Expected bus.shutdown() to have been called");

    // Wait for final flush
    logger_handle.await.unwrap();

    // Confirm that the chat message made it to DB
    let rows = analytics_repo
        .get_recent_messages("plugin_shutdown_test", "chan", 10)
        .await?;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].message_text, "hello from user");

    Ok(())
}

/// Test that after shutdown, no new messages are processed or stored.
#[tokio::test]
async fn test_no_new_events_after_shutdown() -> Result<()> {
    let db = Database::new(":memory:").await?;
    db.migrate().await?;

    let analytics_repo = SqliteAnalyticsRepository::new(db.pool().clone());
    let event_bus = Arc::new(EventBus::new());

    let logger_handle = spawn_db_logger_task(&event_bus, analytics_repo.clone(), 5, 1);

    // Immediately shut down
    event_bus.shutdown();

    // Attempt to publish post-shutdown
    event_bus
        .publish(BotEvent::ChatMessage {
            platform: "unused".to_string(),
            channel: "unused".to_string(),
            user: "unused".to_string(),
            text: "Should not be stored".to_string(),
            timestamp: Utc::now(),
        })
        .await;

    // Wait for final flush
    logger_handle.await.unwrap();

    // Should be zero
    let rows = analytics_repo
        .get_recent_messages("unused", "unused", 10)
        .await?;
    assert_eq!(rows.len(), 0, "No messages should be stored post-shutdown");

    Ok(())
}
