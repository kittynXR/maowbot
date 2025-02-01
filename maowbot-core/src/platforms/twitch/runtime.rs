// File: src/platforms/twitch/runtime.rs

use std::sync::Arc;
use async_trait::async_trait;
use twitch_api::{HelixClient};
use reqwest::Client as ReqwestClient;

use crate::Error;
use crate::models::PlatformCredential;
use crate::platforms::{ConnectionStatus, PlatformAuth, PlatformIntegration};

/// The primary Twitch platform struct. Note that we store:
/// - `Arc<ReqwestClient>` inside `HelixClient` to satisfy `HttpClient` + `'static` lifetime.
pub struct TwitchPlatform {
    pub credentials: Option<PlatformCredential>,
    pub connection_status: ConnectionStatus,

    /// Owned Helix client, requiring `'static` for the lifetime,
    /// and an `Arc<reqwest::Client>` to satisfy the `HttpClient` trait.
    pub client: Option<HelixClient<'static, Arc<ReqwestClient>>>,
}

impl TwitchPlatform {
    /// Example constructor that creates a new reqwest::Client, wraps it in Arc,
    /// and calls `HelixClient::with_client(...)`.
    pub fn new() -> Self {
        // Build a reqwest client, wrap in Arc
        let arc_client = Arc::new(ReqwestClient::new());
        // Then build a HelixClient from that Arc
        let helix_client = HelixClient::with_client(arc_client);

        Self {
            credentials: None,
            connection_status: ConnectionStatus::Disconnected,
            client: Some(helix_client),
        }
    }
}

/// A simple struct representing a chat message event or something similar from Twitch.
pub struct TwitchMessageEvent {
    pub channel: String,
    pub user_id: String,
    pub display_name: String,
    pub text: String,
}

#[async_trait]
impl PlatformAuth for TwitchPlatform {
    async fn authenticate(&mut self) -> Result<(), Error> {
        // E.g. do OAuth or store your credential
        Ok(())
    }

    async fn refresh_auth(&mut self) -> Result<(), Error> {
        // Possibly refresh token using Helix or OAuth
        Ok(())
    }

    async fn revoke_auth(&mut self) -> Result<(), Error> {
        // Clear out credential data
        self.credentials = None;
        // Drop the client if you want to fully "disconnect"
        self.client = None;
        Ok(())
    }

    async fn is_authenticated(&self) -> Result<bool, Error> {
        // If we have credentials, we consider ourselves authenticated
        Ok(self.credentials.is_some())
    }
}

#[async_trait]
impl PlatformIntegration for TwitchPlatform {
    async fn connect(&mut self) -> Result<(), Error> {
        self.connection_status = ConnectionStatus::Connected;
        // Real logic might create an IRC connection or something else
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<(), Error> {
        self.connection_status = ConnectionStatus::Disconnected;
        // Maybe drop IRC connection
        Ok(())
    }

    async fn send_message(&self, _channel: &str, _message: &str) -> Result<(), Error> {
        // Real code might use Twitch IRC or Chat API
        Ok(())
    }

    async fn get_connection_status(&self) -> Result<ConnectionStatus, Error> {
        Ok(self.connection_status.clone())
    }
}

impl TwitchPlatform {
    /// Stub method returning the next “message event,” if any.
    /// Real code might poll IRC events, EventSub, etc.
    pub async fn next_message_event(&mut self) -> Option<TwitchMessageEvent> {
        None
    }
}
