//! src/platforms/twitch_irc/runtime.rs
use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::mpsc::Sender;
use tokio::task::JoinHandle;
use tracing::{debug, error, info, warn};

use crate::Error;
use crate::eventbus::EventBus;
use crate::models::PlatformCredential;
use crate::platforms::{ChatPlatform, ConnectionStatus, PlatformAuth, PlatformIntegration};

use super::client::{TwitchIrcClient, IrcIncomingEvent};

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

    pub client: Option<TwitchIrcClient>,

    /// The read loop that picks from the client's `incoming` channel.
    pub read_loop_handle: Option<JoinHandle<()>>,

    /// Optional event bus if we want to publish chat messages system-wide.
    pub event_bus: Option<Arc<EventBus>>,

    /// We also provide a local channel of `TwitchIrcMessageEvent` for anyone
    /// who wants "next_message_event()".
    pub(crate) rx: Option<tokio::sync::mpsc::Receiver<TwitchIrcMessageEvent>>,
    tx: Option<tokio::sync::mpsc::Sender<TwitchIrcMessageEvent>>,
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
        }
    }

    pub fn set_credentials(&mut self, creds: PlatformCredential) {
        self.credentials = Some(creds);
    }

    pub fn set_event_bus(&mut self, bus: Arc<EventBus>) {
        self.event_bus = Some(bus);
    }

    /// Consumes one message from the local channel (if any).
    pub async fn next_message_event(&mut self) -> Option<TwitchIrcMessageEvent> {
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
        // Typically no-op, we rely on the credential having a valid token.
        Ok(())
    }
    async fn refresh_auth(&mut self) -> Result<(), Error> {
        // If needed. We'll skip here.
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
        if self.client.is_some() {
            info!("(TwitchIrcPlatform) connect => already connected");
            return Ok(());
        }

        let creds = match &self.credentials {
            Some(c) => c,
            None => return Err(Error::Platform("TwitchIRC: No credentials set".into())),
        };
        let token = creds.primary_token.clone();
        if !token.starts_with("oauth:") {
            return Err(Error::Platform("Twitch IRC token must start with 'oauth:'".into()));
        }
        let username = creds.user_name.clone();
        if username.is_empty() {
            return Err(Error::Platform("Twitch IRC credentials missing user_name".into()));
        }

        // Create local channel for sending final TwitchIrcMessageEvent
        let (tx_evt, rx_evt) = tokio::sync::mpsc::channel::<TwitchIrcMessageEvent>(1000);
        self.tx = Some(tx_evt);
        self.rx = Some(rx_evt);

        let client = match TwitchIrcClient::connect(&username, &token).await {
            Ok(c) => c,
            Err(e) => {
                let msg = format!("Error connecting to Twitch IRC => {}", e);
                error!("{}", msg);
                self.connection_status = ConnectionStatus::Error(msg);
                return Err(Error::Platform("Twitch IRC connect failed".into()));
            }
        };
        self.client = Some(client);
        self.connection_status = ConnectionStatus::Connected;

        // Now spawn a read loop that picks from the client's "incoming" channel
        let cli_incoming = self.client.as_mut().unwrap().incoming.take();
        if cli_incoming.is_none() {
            // Should never happen unless connect logic changed
            return Err(Error::Platform("No incoming channel in TwitchIrcClient".into()));
        }
        let mut irc_incoming = cli_incoming.unwrap();
        let tx_for_task = self.tx.as_ref().unwrap().clone();
        let event_bus_for_task = self.event_bus.clone();

        let handle = tokio::spawn(async move {
            while let Some(evt) = irc_incoming.recv().await {
                if evt.command.eq_ignore_ascii_case("privmsg") {
                    let channel = evt.channel.unwrap_or_default();
                    let user_name = evt.user_name.unwrap_or_default();
                    let user_id = evt.user_id.unwrap_or_default();
                    let text = evt.text.unwrap_or_default();

                    let parsed = TwitchIrcMessageEvent {
                        channel,
                        user_name,
                        user_id,
                        text,
                    };

                    let _ = tx_for_task.send(parsed.clone()).await;

                    // Optionally also publish on EventBus
                    if let Some(bus) = &event_bus_for_task {
                        bus.publish_chat(
                            "twitch-irc",
                            &parsed.channel,
                            &parsed.user_name,
                            &parsed.text,
                        ).await;
                    }
                } else {
                    debug!("(TwitchIrcPlatform) ignoring non-PRIVMSG => {:?}", evt.command);
                }
            }
            info!("(TwitchIrcPlatform) read loop ended.");
        });
        self.read_loop_handle = Some(handle);

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
        // Stub
        Ok(vec![])
    }
}
