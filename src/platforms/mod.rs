// src/platforms/mod.rs
use async_trait::async_trait;
use crate::Error;

#[async_trait]
pub trait PlatformIntegration {
    async fn connect(&mut self) -> Result<(), Error>;
    async fn disconnect(&mut self) -> Result<(), Error>;
    async fn send_message(&self, channel: &str, message: &str) -> Result<(), Error>;
}

// Platform-specific traits
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