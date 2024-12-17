// src/platforms/twitch.rs
use async_trait::async_trait;
use crate::Error;
use twitch_api::{HelixClient};
use crate::platforms::{ConnectionStatus, PlatformAuth, PlatformIntegration};
use crate::models::PlatformCredential;

pub struct TwitchPlatform<'a> {
    client: Option<HelixClient<'a, reqwest::Client>>,
    credentials: Option<PlatformCredential>,
    connection_status: ConnectionStatus,
}

#[async_trait]
impl<'a> PlatformAuth for TwitchPlatform<'a> {
    async fn authenticate(&mut self) -> Result<(), Error> {
        // Your existing implementation
        Ok(())
    }

    async fn refresh_auth(&mut self) -> Result<(), Error> {
        // Your existing implementation
        Ok(())
    }

    async fn revoke_auth(&mut self) -> Result<(), Error> {
        // Add this implementation
        self.credentials = None;
        self.client = None;
        Ok(())
    }

    async fn is_authenticated(&self) -> Result<bool, Error> {
        // Add this implementation
        Ok(self.credentials.is_some())
    }
}

#[async_trait]
impl<'a> PlatformIntegration for TwitchPlatform<'a> {
    async fn connect(&mut self) -> Result<(), Error> {
        // Your existing implementation
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<(), Error> {
        // Add this implementation
        self.connection_status = ConnectionStatus::Disconnected;
        Ok(())
    }

    async fn send_message(&self, channel: &str, message: &str) -> Result<(), Error> {
        // Your existing implementation
        Ok(())
    }

    async fn get_connection_status(&self) -> Result<ConnectionStatus, Error> {
        // Add this implementation
        Ok(self.connection_status.clone())
    }
}