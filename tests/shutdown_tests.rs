// tests/shutdown_tests.rs

use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::{sleep, Duration};

use chrono::Utc;
use uuid::Uuid;
use tracing::{info, error};

use anyhow::Result;

// Your crate’s modules
use maowbot::Database;
use maowbot::eventbus::{EventBus, BotEvent};
use maowbot::eventbus::db_logger::spawn_db_logger_task;
use maowbot::plugins::manager::{PluginManager, PluginConnection, PluginConnectionInfo};
use maowbot::plugins::protocol::{PluginToBot, BotToPlugin};
use maowbot::plugins::capabilities::PluginCapability;
use maowbot::repositories::sqlite::analytics::{SqliteAnalyticsRepository, AnalyticsRepo};

use maowbot::Error;  // For the PluginConnection trait
use async_trait::async_trait;

/// A trivial mock plugin that can send a Shutdown message to the bot manager.
#[derive(Clone)]
struct ShutdownTestPlugin {
    info: Arc<Mutex<PluginConnectionInfo>>,
    // Collect events we receive
    received_events: Arc<Mutex<Vec<BotToPlugin>>>,
}

impl ShutdownTestPlugin {
    fn new(name: &str, capabilities: Vec<PluginCapability>) -> Self {
        let info = PluginConnectionInfo {
            name: name.to_string(),
            capabilities,
        };
        Self {
            info: Arc::new(Mutex::new(info)),
            received_events: Arc::new(Mutex::new(vec![])),
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

    async fn send(&self, event: BotToPlugin) -> std::result::Result<(), Error> {
        // We just store it in-memory
        let mut lock = self.received_events.lock().await;
        lock.push(event);
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
async fn test_graceful_shutdown_data_flush() -> anyhow::Result<()> {
    use chrono::Utc;
    use sqlx::query;
    use tokio::time::Duration;
    use tracing::info;

    // 1) In-memory DB
    let db = Database::new(":memory:").await?;
    db.migrate().await?;

    // 2) Insert user rows for user_0, user_1, user_2 so that
    //    chat_messages.user_id = "user_i" passes the foreign key constraint.
    for i in 0..3 {
        let user_id = format!("user_{i}");
        let created_at = Utc::now().naive_utc();
        let last_seen = Utc::now().naive_utc();

        query(r#"
            INSERT INTO users (user_id, created_at, last_seen, is_active)
            VALUES ($1, $2, $3, $4)
        "#)
            .bind(user_id)        // $1
            .bind(created_at)     // $2
            .bind(last_seen)      // $3
            .bind(true)           // $4
            .execute(db.pool())
            .await?;
    }

    // 3) Create analytics repo + event bus
    let analytics_repo = SqliteAnalyticsRepository::new(db.pool().clone());
    let event_bus = Arc::new(EventBus::new());

    // 4) Spawn the DB logger, returning a JoinHandle
    let logger_handle = spawn_db_logger_task(&event_bus, analytics_repo.clone(), 5, 1);

    // 5) Publish some ChatMessage events referencing user_0..user_2
    for i in 0..3 {
        event_bus.publish(BotEvent::ChatMessage {
            platform: "shutdown_test".to_string(),
            channel: "my_channel".to_string(),
            user: format!("user_{i}"),   // must match the inserted row
            text: format!("message number {i}"),
            timestamp: Utc::now(),
        }).await;
    }

    // Optional small delay to let those messages get queued
    tokio::time::sleep(Duration::from_millis(50)).await;

    // 6) Trigger shutdown so the DB logger flushes
    info!("test_graceful_shutdown_data_flush: calling event_bus.shutdown()");
    event_bus.shutdown();

    // 7) Await the logger’s final flush
    logger_handle.await.unwrap();

    // 8) Now read from the DB => we should see all 3
    let rows = analytics_repo.get_recent_messages("shutdown_test", "my_channel", 10).await?;
    assert_eq!(
        rows.len(),
        3,
        "All 3 chat messages should have been flushed"
    );

    Ok(())
}


/// Test that if a plugin sends `PluginToBot::Shutdown`, we also flush everything.
#[tokio::test]
async fn test_plugin_initiated_shutdown() -> anyhow::Result<()> {
    use chrono::Utc;
    use sqlx::query;  // dynamic approach
    use tokio::time::Duration;

    // 1) In-memory DB
    let db = Database::new(":memory:").await?;
    db.migrate().await?;

    // 2) Insert the "some_user" row so that ChatMessages can reference it
    let created_at = Utc::now().naive_utc();
    let last_seen = Utc::now().naive_utc();

    query(r#"
        INSERT INTO users (user_id, created_at, last_seen, is_active)
        VALUES ($1, $2, $3, $4)
    "#)
        .bind("some_user")    // $1
        .bind(created_at)     // $2
        .bind(last_seen)      // $3
        .bind(true)           // $4
        .execute(db.pool())
        .await?;

    // 3) Create repo, event bus, spawn logger
    let analytics_repo = SqliteAnalyticsRepository::new(db.pool().clone());
    let event_bus = Arc::new(EventBus::new());
    let logger_handle = spawn_db_logger_task(&event_bus, analytics_repo.clone(), 5, 1);

    // 4) Create plugin manager
    let mut plugin_mgr = PluginManager::new(None);
    plugin_mgr.set_event_bus(event_bus.clone());
    plugin_mgr.subscribe_to_event_bus(event_bus.clone());

    // 5) Register a plugin that can request shutdown
    let plugin = ShutdownTestPlugin::new("shutdown_initiator", vec![]);
    {
        let mut lock = plugin_mgr.plugins.lock().await;
        lock.push(Arc::new(plugin.clone()));
    }

    // 6) Publish a message referencing user "some_user"
    event_bus.publish(BotEvent::ChatMessage {
        platform: "plugin_shutdown_test".to_string(),
        channel: "chan".to_string(),
        user: "some_user".to_string(),
        text: "hello from user".to_string(),
        timestamp: Utc::now(),
    }).await;

    // (Optional) short delay so the message is definitely in the queue
    tokio::time::sleep(Duration::from_millis(50)).await;

    // 7) The plugin triggers a Shutdown => manager calls event_bus.shutdown()
    plugin_mgr.on_plugin_message(
        PluginToBot::Shutdown,
        "shutdown_initiator",
        Arc::new(plugin.clone())
    ).await;

    // 8) Confirm event_bus is shut down
    assert!(event_bus.is_shutdown());

    // 9) Wait for logger to finish final flush
    logger_handle.await.unwrap();

    // 10) Confirm row is in DB
    let rows = analytics_repo.get_recent_messages("plugin_shutdown_test", "chan", 10).await?;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].message_text, "hello from user");

    Ok(())
}



/// Test that after shutdown, no new messages are processed.
#[tokio::test]
async fn test_no_new_events_after_shutdown() -> Result<()> {
    let db = Database::new(":memory:").await?;
    db.migrate().await?;

    let analytics_repo = SqliteAnalyticsRepository::new(db.pool().clone());
    let event_bus = Arc::new(EventBus::new());
    let logger_handle = spawn_db_logger_task(&event_bus, analytics_repo.clone(), 5, 1);

    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    // Immediately shut down
    event_bus.shutdown();

    // Attempt to publish post-shutdown
    event_bus.publish(BotEvent::ChatMessage {
        platform: "unused".to_string(),
        channel: "unused".to_string(),
        user: "unused".to_string(),
        text: "Should not be stored".to_string(),
        timestamp: Utc::now(),
    }).await;


    // Wait for final flush
    logger_handle.await.unwrap();

    // Should be zero
    let rows = analytics_repo.get_recent_messages("unused", "unused", 10).await?;
    assert_eq!(rows.len(), 0, "No messages should be stored post-shutdown");

    Ok(())
}
