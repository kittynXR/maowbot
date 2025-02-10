use async_trait::async_trait;
use crate::Error;
use crate::models::PlatformCredential;
use crate::platforms::{ConnectionStatus, PlatformAuth, PlatformIntegration};

pub struct VRChatPlatform {
    pub(crate) credentials: Option<PlatformCredential>,
    pub(crate) connection_status: ConnectionStatus,
}

impl VRChatPlatform {
    pub fn new() -> Self {
        Self {
            credentials: None,
            connection_status: ConnectionStatus::Disconnected,
        }
    }
}

/// Example event struct
pub struct VRChatMessageEvent {
    pub vrchat_display_name: String,
    pub user_id: String,
    pub text: String,
}

#[async_trait]
impl PlatformAuth for VRChatPlatform {
    async fn authenticate(&mut self) -> Result<(), Error> {
        Ok(())
    }
    async fn refresh_auth(&mut self) -> Result<(), Error> { Ok(()) }
    async fn revoke_auth(&mut self) -> Result<(), Error> {
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
        self.connection_status = ConnectionStatus::Connected;
        // ...
        Ok(())
    }
    async fn disconnect(&mut self) -> Result<(), Error> {
        self.connection_status = ConnectionStatus::Disconnected;
        Ok(())
    }
    async fn send_message(&self, _channel: &str, _message: &str) -> Result<(), Error> {
        // VRChat might not truly support text the same way. Stub out
        Ok(())
    }
    async fn get_connection_status(&self) -> Result<ConnectionStatus, Error> {
        Ok(self.connection_status.clone())
    }
}

impl VRChatPlatform {
    pub async fn next_message_event(&mut self) -> Option<VRChatMessageEvent> {
        // Stub
        None
    }
}
