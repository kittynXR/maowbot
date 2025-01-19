// File: src/platforms/vrchat/runtime.rs

use async_trait::async_trait;
use crate::platforms::{ConnectionStatus, PlatformAuth, PlatformIntegration};
use crate::Error;
use crate::models::PlatformCredential;

pub struct VRChatPlatform {
    credentials: Option<PlatformCredential>,
    connection_status: ConnectionStatus,
}

#[async_trait]
impl PlatformAuth for VRChatPlatform {
    async fn authenticate(&mut self) -> Result<(), Error> {
        // ...
        Ok(())
    }

    async fn refresh_auth(&mut self) -> Result<(), Error> {
        // ...
        Ok(())
    }

    async fn revoke_auth(&mut self) -> Result<(), Error> {
        // ...
        self.credentials = None;
        Ok(())
    }

    async fn is_authenticated(&self) -> Result<bool, Error> {
        Ok(self.credentials.is_some())
    }
}

#[async_trait]
impl PlatformIntegration for VRChatPlatform {
    async fn connect(&mut self) -> Result<(), Error> {
        // ...
        self.connection_status = ConnectionStatus::Connected;
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<(), Error> {
        self.connection_status = ConnectionStatus::Disconnected;
        Ok(())
    }

    async fn send_message(&self, _channel: &str, _message: &str) -> Result<(), Error> {
        // VRChat might not have the same concept of “channel”
        Ok(())
    }

    async fn get_connection_status(&self) -> Result<ConnectionStatus, Error> {
        Ok(self.connection_status.clone())
    }
}
