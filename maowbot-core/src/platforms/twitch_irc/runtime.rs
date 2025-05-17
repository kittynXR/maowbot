use async_trait::async_trait;
use std::sync::Arc;
use tokio::task::JoinHandle;
use tracing::{debug, error, info};

use crate::Error;
use crate::eventbus::EventBus;
use maowbot_common::models::platform::PlatformCredential;
use maowbot_common::traits::platform_traits::{ChatPlatform, ConnectionStatus, PlatformAuth, PlatformIntegration};

use super::client::{TwitchIrcClient, IrcIncomingEvent};

#[derive(Debug, Clone)]
pub struct TwitchIrcMessageEvent {
    pub channel: String,
    /// The numeric user-id from Twitch, e.g. "264653338"
    pub twitch_user_id: String,
    /// The user’s display name from “display-name” or prefix
    pub display_name: String,
    pub text: String,
    pub roles: Vec<String>,
}

pub struct TwitchIrcPlatform {
    pub credentials: Option<PlatformCredential>,
    pub connection_status: ConnectionStatus,

    pub client: Option<TwitchIrcClient>,
    pub read_loop_handle: Option<JoinHandle<()>>,
    pub event_bus: Option<Arc<EventBus>>,

    /// A local channel for `TwitchIrcMessageEvent`.
    pub(crate) rx: Option<tokio::sync::mpsc::Receiver<TwitchIrcMessageEvent>>,
    tx: Option<tokio::sync::mpsc::Sender<TwitchIrcMessageEvent>>,

    /// **NEW**: If false, we skip reading/processing incoming messages.
    /// This is how we differentiate broadcaster vs. bot accounts.
    pub enable_incoming: bool,
}

impl TwitchIrcPlatform {
    pub fn new() -> Self {
        Self {
            credentials: None,
            connection_status: ConnectionStatus::Disconnected,
            client: None,
            read_loop_handle: None,
            event_bus: None,
            rx: None,
            tx: None,
            enable_incoming: true, // default
        }
    }

    pub fn set_credentials(&mut self, creds: PlatformCredential) {
        self.credentials = Some(creds);
    }

    pub fn set_event_bus(&mut self, bus: Arc<EventBus>) {
        self.event_bus = Some(bus);
    }

    /// Helper to consume next message event if this platform is in "receive" mode
    pub async fn next_message_event(&mut self) -> Option<TwitchIrcMessageEvent> {
        if !self.enable_incoming {
            return None; // do nothing if reading is disabled
        }
        if let Some(rx_ref) = &mut self.rx {
            rx_ref.recv().await
        } else {
            None
        }
    }
}

#[async_trait]
impl PlatformAuth for TwitchIrcPlatform {
    async fn authenticate(&mut self) -> Result<(), Error> {
        Ok(())
    }
    async fn refresh_auth(&mut self) -> Result<(), Error> {
        Ok(())
    }
    async fn revoke_auth(&mut self) -> Result<(), Error> {
        self.credentials = None;
        Ok(())
    }
    async fn is_authenticated(&self) -> Result<bool, Error> {
        Ok(self.credentials.is_some())
    }
}

#[async_trait]
impl PlatformIntegration for TwitchIrcPlatform {
    async fn connect(&mut self) -> Result<(), Error> {
        // Already connected?
        if self.client.is_some() {
            info!("(TwitchIrcPlatform) connect ⇒ already connected");
            return Ok(());
        }

        // ------------------------------------------------------------------
        // 1) Load creds and refresh if they are (almost) expired.
        // ------------------------------------------------------------------
        let mut creds = self
            .credentials
            .clone()
            .ok_or_else(|| Error::Platform("TwitchIRC: No credentials set".into()))?;

        // Extract client-id from additional_data OR fall back to env var.
        let client_id = creds
            .additional_data
            .as_ref()
            .and_then(|v| v.get("client_id"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .or_else(|| std::env::var("TWITCH_CLIENT_ID").ok())
            .unwrap_or_default();

        let client_secret = std::env::var("TWITCH_CLIENT_SECRET").ok();

        // Refresh if needed (≤ 10 min left).
        creds = crate::platforms::twitch::requests::token::ensure_valid_token(
            &creds,
            &client_id,
            client_secret.as_deref(),
            600,
        )
            .await?;
        // Persist into the struct so downstream logic uses the fresh token.
        self.credentials = Some(creds.clone());

        // ------------------------------------------------------------------
        // 2) Validate basic shape & connect.
        // ------------------------------------------------------------------
        let token = creds.primary_token.clone();
        if !token.starts_with("oauth:") {
            return Err(Error::Platform(
                "Twitch IRC token must start with 'oauth:'".into(),
            ));
        }
        let username = creds.user_name.clone();
        if username.is_empty() {
            return Err(Error::Platform("Twitch IRC credential missing user_name".into()));
        }

        let (tx_evt, rx_evt) = tokio::sync::mpsc::channel::<TwitchIrcMessageEvent>(1000);
        self.tx = Some(tx_evt);
        self.rx = Some(rx_evt);

        // Underlying TCP + TLS connect.
        let client = TwitchIrcClient::connect(&username, &token).await.map_err(|e| {
            let msg = format!("Error connecting to Twitch IRC ⇒ {}", e);
            error!("{}", msg);
            Error::Platform(msg)
        })?;
        self.client = Some(client);
        self.connection_status = ConnectionStatus::Connected;

        // --- spawn read-loop exactly as before (unchanged code) -------------
        if self.enable_incoming {
            let mut irc_incoming = self
                .client
                .as_mut()
                .unwrap()
                .incoming
                .take()
                .ok_or_else(|| Error::Platform("No incoming channel in TwitchIrcClient".into()))?;

            let tx_for_task = self.tx.as_ref().unwrap().clone();
            let event_bus_for_task = self.event_bus.clone();

            let handle = tokio::spawn(async move {
                while let Some(evt) = irc_incoming.recv().await {
                    // … existing PRIVMSG handling …
                    if evt.command.eq_ignore_ascii_case("privmsg") {
                        if evt.twitch_user_id.is_none() {
                            debug!("Skipping message without user-id ⇒ {:?}", evt.raw_line);
                            continue;
                        }
                        let msg_evt = TwitchIrcMessageEvent {
                            channel:      evt.channel.clone().unwrap_or_default(),
                            twitch_user_id: evt.twitch_user_id.clone().unwrap_or_default(),
                            display_name: evt
                                .display_name
                                .clone()
                                .unwrap_or_else(|| "<unknown>".into()),
                            text:  evt.text.clone().unwrap_or_default(),
                            roles: evt.roles.clone(),
                        };
                        let _ = tx_for_task.send(msg_evt).await;
                        // (optional event-bus publish unchanged)
                    }
                }
                info!("(TwitchIrcPlatform) read loop ended.");
            });
            self.read_loop_handle = Some(handle);
        } else {
            info!("(TwitchIrcPlatform) incoming-chat disabled for this account (bot mode)");
        }

        Ok(())
    }

    async fn disconnect(&mut self) -> Result<(), Error> {
        self.connection_status = ConnectionStatus::Disconnected;

        if let Some(cli) = self.client.take() {
            cli.shutdown();
        }
        if let Some(h) = self.read_loop_handle.take() {
            h.abort();
        }
        Ok(())
    }

    async fn send_message(&self, channel: &str, message: &str) -> Result<(), Error> {
        if let Some(cli) = &self.client {
            cli.send_privmsg(channel, message);
            Ok(())
        } else {
            Err(Error::Platform("No active Twitch IRC connection".into()))
        }
    }

    async fn get_connection_status(&self) -> Result<ConnectionStatus, Error> {
        Ok(self.connection_status.clone())
    }
}

#[async_trait]
impl ChatPlatform for TwitchIrcPlatform {
    async fn join_channel(&self, channel: &str) -> Result<(), Error> {
        if let Some(cli) = &self.client {
            cli.join_channel(channel);
            Ok(())
        } else {
            Err(Error::Platform("No active IRC client connection".into()))
        }
    }

    async fn leave_channel(&self, channel: &str) -> Result<(), Error> {
        if let Some(cli) = &self.client {
            cli.part_channel(channel);
            Ok(())
        } else {
            Err(Error::Platform("No active IRC client connection".into()))
        }
    }

    async fn get_channel_users(&self, _channel: &str) -> Result<Vec<String>, Error> {
        // (Not implemented in this snippet.)
        Ok(vec![])
    }
}