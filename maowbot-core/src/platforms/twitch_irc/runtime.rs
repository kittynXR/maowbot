use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender, Receiver};
use tokio::task::JoinHandle;
use tracing::{error, info, warn};

use twitch_irc::message::{ServerMessage, PrivmsgMessage};
use twitch_irc::{ClientConfig, SecureTCPTransport, TwitchIRCClient};
use twitch_irc::login::StaticLoginCredentials;

use crate::Error;
use crate::models::PlatformCredential;
use crate::platforms::{ConnectionStatus, PlatformAuth, PlatformIntegration};

/// Example “message event” from Twitch IRC
#[derive(Debug, Clone)]
pub struct TwitchIrcMessageEvent {
    pub channel: String,
    pub user_name: String,
    pub user_id: String,   // In real usage, you might do a Helix lookup to get numeric user_id
    pub text: String,
}

/// Our platform struct.
/// - `credentials`: the chat token
/// - a local `connection_status`
/// - an optional internal read task handle
/// - an internal channel for feeding parsed events
pub struct TwitchIrcPlatform {
    pub credentials: Option<PlatformCredential>,
    pub connection_status: ConnectionStatus,

    pub read_task_handle: Option<JoinHandle<()>>,
    pub rx: Option<UnboundedReceiver<TwitchIrcMessageEvent>>,
    pub tx: Option<UnboundedSender<TwitchIrcMessageEvent>>,
}

impl TwitchIrcPlatform {
    pub fn new() -> Self {
        Self {
            credentials: None,
            connection_status: ConnectionStatus::Disconnected,
            read_task_handle: None,
            rx: None,
            tx: None,
        }
    }

    /// If your manager wants to poll for messages, it calls `next_message_event()`.
    /// This returns `None` if we’re disconnected or the channel is closed.
    pub async fn next_message_event(&mut self) -> Option<TwitchIrcMessageEvent> {
        if let Some(rx) = &mut self.rx {
            rx.recv().await
        } else {
            None
        }
    }
}

/// Because fields are private, we provide getters/setters if needed.
impl TwitchIrcPlatform {
    pub fn set_credentials(&mut self, creds: PlatformCredential) {
        self.credentials = Some(creds);
    }

    pub fn credentials(&self) -> Option<&PlatformCredential> {
        self.credentials.as_ref()
    }
}

#[async_trait]
impl PlatformAuth for TwitchIrcPlatform {
    async fn authenticate(&mut self) -> Result<(), Error> {
        if self.credentials.is_none() {
            return Err(Error::Auth("No credentials set for Twitch IRC".into()));
        }
        Ok(())
    }

    async fn refresh_auth(&mut self) -> Result<(), Error> {
        // e.g. manager calls into auth manager if needed
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
        if self.connection_status == ConnectionStatus::Connected {
            warn!("(TwitchIrcPlatform) Already connected; skipping.");
            return Ok(());
        }

        let creds = match &self.credentials {
            Some(c) => c,
            None => return Err(Error::Auth("No credentials in TwitchIrcPlatform".into())),
        };

        // The token in DB might be stored as "oauth:XXXXXXXX". The library expects just "XXXXXXXX".
        let raw_token = creds
            .primary_token
            .strip_prefix("oauth:")
            .unwrap_or(&creds.primary_token)
            .to_string();

        // If you know the user’s actual Twitch login, store it in additional_data or in the credential.
        // For demo, use a placeholder:
        let user_login_name = "someNickName";

        let config = ClientConfig::new_simple(
            StaticLoginCredentials::new(user_login_name.to_string(), Some(raw_token))
        );

        // We’ll use SecureTCPTransport for TLS connections.
        let (mut incoming_messages, client) = TwitchIRCClient::<SecureTCPTransport, _>::new(config);

        // Create an unbounded channel for parsed message events
        let (tx, rx) = unbounded_channel();
        self.tx = Some(tx);
        self.rx = Some(rx);

        // Start a background task reading from `incoming_messages`:
        let tx_for_task = self.tx.clone();
        let join_handle = tokio::spawn(async move {
            while let Some(msg) = incoming_messages.recv().await {
                match msg {
                    ServerMessage::Privmsg(privmsg) => {
                        let evt = TwitchIrcMessageEvent {
                            channel: privmsg.channel_login,  // e.g. "somechannel"
                            user_name: privmsg.sender.login.clone(),
                            user_id: privmsg.sender.id.clone(),
                            text: privmsg.message_text.clone(),
                        };
                        if let Some(sender) = &tx_for_task {
                            let _ = sender.send(evt);
                        }
                    }
                    // handle other messages if you want
                    _ => {}
                }
            }
            info!("(TwitchIrcPlatform) IRC read task ended.");
        });
        self.read_task_handle = Some(join_handle);

        // Join whichever channel you need:
        client.join("somechannel".to_string());

        self.connection_status = ConnectionStatus::Connected;
        info!("(TwitchIrcPlatform) connected with user_id={}", creds.user_id);
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<(), Error> {
        self.connection_status = ConnectionStatus::Disconnected;
        if let Some(h) = self.read_task_handle.take() {
            h.abort();
        }
        self.rx = None;
        self.tx = None;
        Ok(())
    }

    async fn send_message(&self, channel: &str, message: &str) -> Result<(), Error> {
        // If you want to call `.say(channel, message)` from the `client`, store an Arc<TwitchIRCClient<_,_>> in self.
        // This snippet does not keep it around, so we can’t directly send. :-)
        warn!("(TwitchIrcPlatform) send_message not yet implemented in snippet!");
        Ok(())
    }

    async fn get_connection_status(&self) -> Result<ConnectionStatus, Error> {
        Ok(self.connection_status.clone())
    }
}