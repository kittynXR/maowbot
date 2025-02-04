// File: maowbot-core/tests/integration/shutdown_tests.rs

use std::sync::Arc;
use tokio::time::Duration;
use chrono::Utc;
use sqlx::Executor;

use maowbot_core::{
    db::Database,
    Error,
    eventbus::{EventBus, BotEvent},
    eventbus::db_logger::spawn_db_logger_task,
    plugins::manager::{PluginManager, PluginConnection, PluginConnectionInfo},
    repositories::postgres::analytics::{PostgresAnalyticsRepository, AnalyticsRepo},
};
use maowbot_proto::plugs::{
    plugin_stream_request::Payload as ReqPayload,
    plugin_stream_response::Payload as RespPayload,
    PluginCapability,
    Shutdown,
};
use async_trait::async_trait;

use maowbot_core::test_utils::helpers::setup_test_database;

#[derive(Clone)]
struct ShutdownTestPlugin {
    info: Arc<tokio::sync::Mutex<PluginConnectionInfo>>,
}

impl ShutdownTestPlugin {
    fn new(name: &str, capabilities: Vec<PluginCapability>) -> Self {
        let info = PluginConnectionInfo {
            name: name.to_string(),
            capabilities,
            is_enabled: true,
        };
        Self {
            info: Arc::new(tokio::sync::Mutex::new(info)),
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

    async fn set_enabled(&self, enabled: bool) {
        let mut info = self.info.lock().await;
        info.is_enabled = enabled;
    }

    async fn send(&self, _response: maowbot_proto::plugs::PluginStreamResponse) -> Result<(), Error> {
        Ok(())
    }

    async fn stop(&self) -> Result<(), Error> {
        Ok(())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[tokio::test]
async fn test_graceful_shutdown_data_flush() -> Result<(), Error> {
    let db = setup_test_database().await?;

    // Insert some test users
    {
        let mut tx = db.pool().begin().await?;
        for i in 0..3 {
            let user_id = format!("user_{i}");
            tx.execute(
                sqlx::query(
                    r#"INSERT INTO users (user_id, created_at, last_seen, is_active)
                       VALUES ($1, NOW(), NOW(), TRUE)"#
                )
                    .bind(user_id)
            )
                .await?;
        }
        tx.commit().await?;
    }

    let analytics_repo = PostgresAnalyticsRepository::new(db.pool().clone());
    let event_bus = Arc::new(EventBus::new());
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
    event_bus.shutdown();
    logger_handle.await.unwrap();

    // Confirm all 3 messages were stored
    let rows = analytics_repo
        .get_recent_messages("shutdown_test", "my_channel", 10)
        .await?;
    assert_eq!(rows.len(), 3);

    Ok(())
}

#[tokio::test]
async fn test_plugin_initiated_shutdown() -> Result<(), Error> {
    let db = setup_test_database().await?;

    {
        let mut tx = db.pool().begin().await?;
        tx.execute(
            sqlx::query(
                r#"INSERT INTO users (user_id, created_at, last_seen, is_active)
                   VALUES ('some_user', NOW(), NOW(), TRUE)"#
            )
        )
            .await?;
        tx.commit().await?;
    }

    let analytics_repo = PostgresAnalyticsRepository::new(db.pool().clone());
    let event_bus = Arc::new(EventBus::new());
    let logger_handle = spawn_db_logger_task(&event_bus, analytics_repo.clone(), 5, 1);

    let mut plugin_mgr = PluginManager::new(None);
    plugin_mgr.set_event_bus(event_bus.clone());
    plugin_mgr.subscribe_to_event_bus(event_bus.clone());

    let plugin = ShutdownTestPlugin::new("shutdown_initiator", vec![]);
    {
        let mut lock = plugin_mgr.plugins.lock().await;
        lock.push(Arc::new(plugin.clone()));
    }

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

    // The plugin initiates a shutdown
    plugin_mgr
        .on_inbound_message(
            ReqPayload::Shutdown(Shutdown {}),
            Arc::new(plugin.clone())
        )
        .await;

    // Verify event bus is stopped
    assert!(event_bus.is_shutdown());
    logger_handle.await.unwrap();

    // Confirm the one stored message
    let rows = analytics_repo
        .get_recent_messages("plugin_shutdown_test", "chan", 10)
        .await?;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].message_text, "hello from user");

    Ok(())
}

#[tokio::test]
async fn test_no_new_events_after_shutdown() -> Result<(), Error> {
    let db = setup_test_database().await?;

    let analytics_repo = PostgresAnalyticsRepository::new(db.pool().clone());
    let event_bus = Arc::new(EventBus::new());
    let logger_handle = spawn_db_logger_task(&event_bus, analytics_repo.clone(), 5, 1);

    // Immediately shut down
    event_bus.shutdown();

    // Attempt to publish an event
    event_bus
        .publish(BotEvent::ChatMessage {
            platform: "unused".to_string(),
            channel: "unused".to_string(),
            user: "unused".to_string(),
            text: "Should not be stored".to_string(),
            timestamp: Utc::now(),
        })
        .await;

    // Wait for logger task
    logger_handle.await.unwrap();

    // Confirm nothing was stored
    let rows = analytics_repo
        .get_recent_messages("unused", "unused", 10)
        .await?;
    assert_eq!(rows.len(), 0);

    Ok(())
}