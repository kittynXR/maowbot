// File: src/platforms/mod.rs

use async_trait::async_trait;
use crate::Error;

#[derive(Debug, Clone, PartialEq)]
pub enum ConnectionStatus {
    Connected,
    Disconnected,
    Reconnecting,
    Error(String),
}

#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait PlatformAuth {
    async fn authenticate(&mut self) -> Result<(), Error>;
    async fn refresh_auth(&mut self) -> Result<(), Error>;
    async fn revoke_auth(&mut self) -> Result<(), Error>;
    async fn is_authenticated(&self) -> Result<bool, Error>;
}

#[async_trait]
pub trait PlatformIntegration: PlatformAuth {
    async fn connect(&mut self) -> Result<(), Error>;
    async fn disconnect(&mut self) -> Result<(), Error>;
    async fn send_message(&self, channel: &str, message: &str) -> Result<(), Error>;
    async fn get_connection_status(&self) -> Result<ConnectionStatus, Error>;
}

#[async_trait]
pub trait ChatPlatform: PlatformIntegration {
    async fn join_channel(&self, channel: &str) -> Result<(), Error>;
    async fn leave_channel(&self, channel: &str) -> Result<(), Error>;
    async fn get_channel_users(&self, channel: &str) -> Result<Vec<String>, Error>;
}

#[async_trait]
pub trait StreamingPlatform: PlatformIntegration {
    async fn get_stream_status(&self, channel: &str) -> Result<bool, Error>;
    async fn get_viewer_count(&self, channel: &str) -> Result<u32, Error>;
    async fn update_stream_title(&self, title: &str) -> Result<(), Error>;
}

#[async_trait]
pub trait VirtualPlatform: PlatformIntegration {
    async fn get_world_info(&self) -> Result<String, Error>;
    async fn get_instance_users(&self) -> Result<Vec<String>, Error>;
}

// Re-export submodules
pub mod twitch_helix;
pub mod discord;
pub mod vrchat;
pub mod manager;
pub mod twitch_irc;
pub mod twitch_eventsub;
