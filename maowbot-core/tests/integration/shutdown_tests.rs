// tests/integration/shutdown_tests.rs

use std::sync::Arc;
use tokio::time::Duration;
use chrono::Utc;
use sqlx::Executor;
use maowbot_core::{Database, Error};
use maowbot_core::eventbus::{EventBus, BotEvent};
use maowbot_core::eventbus::db_logger::spawn_db_logger_task;
use maowbot_core::plugins::manager::{
    PluginManager, PluginConnection, PluginConnectionInfo
};
use maowbot_proto::plugs::plugin_stream_request::Payload as ReqPayload;
use maowbot_proto::plugs::{PluginCapability, PluginStreamResponse, plugin_stream_response::Payload as RespPayload, Shutdown};
use maowbot_core::repositories::postgres::analytics::{PostgresAnalyticsRepository, AnalyticsRepo};
use async_trait::async_trait;

#[derive(Clone)]
struct ShutdownTestPlugin {
    info: Arc<tokio::sync::Mutex<PluginConnectionInfo>>,
}

impl ShutdownTestPlugin {
    fn new(name: &str, capabilities: Vec<PluginCapability>) -> Self {
        let info = PluginConnectionInfo {
            name: name.to_string(),
            capabilities,
            is_enabled: false,
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

    async fn send(&self, response: PluginStreamResponse) -> Result<(), Error> {
        // For this test, we do nothing with the response
        let _ = response;
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
    let db = Database::new(":memory:").await?;
    db.migrate().await?;

    {
        let mut tx = db.pool().begin().await?;
        for i in 0..3 {
            let user_id = format!("user_{i}");
            tx.execute(
                sqlx::query(
                    r#"INSERT INTO users (user_id, created_at, last_seen, is_active)
                       VALUES ($1, strftime('%s','now'), strftime('%s','now'), 1)"#
                )
                    .bind(user_id)
            ).await?;
        }
        tx.commit().await?;
    }

    let analytics_repo = PostgresAnalyticsRepository::new(db.pool().clone());
    let event_bus = Arc::new(EventBus::new());
    let logger_handle = spawn_db_logger_task(&event_bus, analytics_repo.clone(), 5, 1);

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

    let rows = analytics_repo
        .get_recent_messages("shutdown_test", "my_channel", 10)
        .await?;
    assert_eq!(rows.len(), 3);

    Ok(())
}

#[tokio::test]
async fn test_plugin_initiated_shutdown() -> Result<(), Error> {
    let db = Database::new(":memory:").await?;
    db.migrate().await?;

    {
        let mut tx = db.pool().begin().await?;
        tx.execute(
            sqlx::query(
                r#"INSERT INTO users (user_id, created_at, last_seen, is_active)
                   VALUES ('some_user', strftime('%s','now'), strftime('%s','now'), 1)"#
            )
        ).await?;
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

    plugin_mgr
        .on_inbound_message(
            ReqPayload::Shutdown(Shutdown {}),
            Arc::new(plugin.clone())
        )
        .await;

    assert!(event_bus.is_shutdown());
    logger_handle.await.unwrap();

    let rows = analytics_repo
        .get_recent_messages("plugin_shutdown_test", "chan", 10)
        .await?;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].message_text, "hello from user");

    Ok(())
}

#[tokio::test]
async fn test_no_new_events_after_shutdown() -> Result<(), Error> {
    let db = Database::new(":memory:").await?;
    db.migrate().await?;

    let analytics_repo = PostgresAnalyticsRepository::new(db.pool().clone());
    let event_bus = Arc::new(EventBus::new());
    let logger_handle = spawn_db_logger_task(&event_bus, analytics_repo.clone(), 5, 1);

    event_bus.shutdown();

    event_bus
        .publish(BotEvent::ChatMessage {
            platform: "unused".to_string(),
            channel: "unused".to_string(),
            user: "unused".to_string(),
            text: "Should not be stored".to_string(),
            timestamp: Utc::now(),
        })
        .await;

    logger_handle.await.unwrap();

    let rows = analytics_repo
        .get_recent_messages("unused", "unused", 10)
        .await?;
    assert_eq!(rows.len(), 0);

    Ok(())
}