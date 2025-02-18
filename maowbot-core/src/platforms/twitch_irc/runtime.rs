use async_trait::async_trait;
use std::sync::Arc;
use tokio::task::JoinHandle;
use tracing::{debug, error, info, warn};

use crate::Error;
use crate::eventbus::EventBus;
use crate::models::PlatformCredential;
use crate::platforms::{ChatPlatform, ConnectionStatus, PlatformAuth, PlatformIntegration};

/// Represents a chat message event on Twitch.
/// In this stubbed version, it's just a placeholder struct.
#[derive(Debug, Clone)]
pub struct TwitchIrcMessageEvent {
    pub channel: String,
    pub user_name: String,
    pub user_id: String,
    pub text: String,
}

pub struct TwitchIrcPlatform {
    pub credentials: Option<PlatformCredential>,
    pub connection_status: ConnectionStatus,

    pub read_task_handle: Option<JoinHandle<()>>,

    /// Stubbed out client. No real Twitch IRC references here.
    pub client: Option<()>,

    /// Optional event bus for publishing chat messages (e.g. for a TUI).
    pub event_bus: Option<Arc<EventBus>>,
}

impl TwitchIrcPlatform {
    /// Creates a new (stub) TwitchIrcPlatform with no actual IRC functionality.
    pub fn new() -> Self {
        Self {
            credentials: None,
            connection_status: ConnectionStatus::Disconnected,
            read_task_handle: None,
            client: None,
            event_bus: None,
        }
    }

    /// Sets credentials (not actually used in this stub).
    pub fn set_credentials(&mut self, creds: PlatformCredential) {
        self.credentials = Some(creds);
    }

    /// Returns a reference to the credentials if they exist.
    pub fn credentials(&self) -> Option<&PlatformCredential> {
        self.credentials.as_ref()
    }

    pub fn set_event_bus(&mut self, bus: Arc<EventBus>) {
        self.event_bus = Some(bus);
    }

    pub async fn next_message_event(&mut self) -> Option<TwitchIrcMessageEvent> {
        None
    }
}

#[async_trait]
impl PlatformAuth for TwitchIrcPlatform {
    async fn authenticate(&mut self) -> Result<(), Error> {
        // Stub: Do nothing.
        Ok(())
    }

    async fn refresh_auth(&mut self) -> Result<(), Error> {
        // Stub: Do nothing.
        Ok(())
    }

    async fn revoke_auth(&mut self) -> Result<(), Error> {
        // Stub: Clear credentials.
        self.credentials = None;
        Ok(())
    }

    async fn is_authenticated(&self) -> Result<bool, Error> {
        // Stub: Return whether credentials are present.
        Ok(self.credentials.is_some())
    }
}

#[async_trait]
impl PlatformIntegration for TwitchIrcPlatform {
    async fn connect(&mut self) -> Result<(), Error> {
        info!("(TwitchIrcPlatform) Connected (stub).");
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<(), Error> {
        // Stub: Cancel any read task and clear channels.
        info!("(TwitchIrcPlatform) Disconnected (stub).");
        Ok(())
    }

    async fn send_message(&self, channel: &str, message: &str) -> Result<(), Error> {
        // Stub: No real sending logic.
        debug!("(TwitchIrcPlatform) send_message => {}: {} (stub)", channel, message);
        Ok(())
    }

    async fn get_connection_status(&self) -> Result<ConnectionStatus, Error> {
        Ok(self.connection_status.clone())
    }
}

#[async_trait]
impl ChatPlatform for TwitchIrcPlatform {
    async fn join_channel(&self, channel: &str) -> Result<(), Error> {
        // Stub: No actual join logic.
        info!("(TwitchIrcPlatform) join_channel => '{}' (stub)", channel);
        Ok(())
    }

    async fn leave_channel(&self, channel: &str) -> Result<(), Error> {
        // Stub: No actual leave logic.
        info!("(TwitchIrcPlatform) leave_channel => '{}' (stub)", channel);
        Ok(())
    }

    async fn get_channel_users(&self, _channel: &str) -> Result<Vec<String>, Error> {
        // Stub: Always return an empty list.
        Ok(Vec::new())
    }
}
