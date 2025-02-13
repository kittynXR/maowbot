use async_trait::async_trait;
use std::sync::Arc;
use futures_util::TryFutureExt;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
use tokio::task::JoinHandle;
use tracing::{error, info, warn};

use twitch_irc::login::StaticLoginCredentials;
use twitch_irc::message::ServerMessage;
use twitch_irc::{ClientConfig, SecureTCPTransport, TwitchIRCClient};

use crate::Error;
use crate::models::PlatformCredential;
use crate::platforms::{
    ConnectionStatus, PlatformAuth, PlatformIntegration, ChatPlatform,
};

/// Example “message event” from Twitch IRC
#[derive(Debug, Clone)]
pub struct TwitchIrcMessageEvent {
    pub channel: String,
    pub user_name: String,
    pub user_id: String,   // (Might just be user’s numeric ID from Twitch.)
    pub text: String,
}

/// Our platform struct for Twitch IRC.
pub struct TwitchIrcPlatform {
    /// The chat token, etc.
    pub credentials: Option<PlatformCredential>,

    /// Current connection status (Connected/Disconnected/Reconnecting/etc.)
    pub connection_status: ConnectionStatus,

    /// A background read task handle that receives ServerMessage from the client
    pub read_task_handle: Option<JoinHandle<()>>,

    /// For piping message events out of the read loop
    pub rx: Option<UnboundedReceiver<TwitchIrcMessageEvent>>,
    pub tx: Option<UnboundedSender<TwitchIrcMessageEvent>>,

    /// A reference to the actual IRC client so we can join/part channels.
    /// We wrap it in `Arc` because we might share it with the read task.
    pub client: Option<Arc<TwitchIRCClient<SecureTCPTransport, StaticLoginCredentials>>>,
}

impl TwitchIrcPlatform {
    pub fn new() -> Self {
        Self {
            credentials: None,
            connection_status: ConnectionStatus::Disconnected,
            read_task_handle: None,
            rx: None,
            tx: None,
            client: None,
        }
    }

    /// If your manager wants to poll for messages, it calls `next_message_event()`.
    /// Returns `None` if we’re disconnected or the channel is closed.
    pub async fn next_message_event(&mut self) -> Option<TwitchIrcMessageEvent> {
        if let Some(rx) = &mut self.rx {
            rx.recv().await
        } else {
            None
        }
    }

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
        // e.g. manager calls into an AuthManager if needed
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

        // The token in DB might be "oauth:XXXXX", library expects just "XXXXX".
        let raw_token = creds
            .primary_token
            .strip_prefix("oauth:")
            .unwrap_or(&creds.primary_token)
            .to_string();

        // Some nickname for the bot. If your credential has more data (like the actual login),
        // you can store it in the credential’s `additional_data`.
        let user_login_name = "someNickName";

        let config = ClientConfig::new_simple(
            StaticLoginCredentials::new(user_login_name.to_string(), Some(raw_token))
        );

        // Create the client
        let (mut incoming_messages, client) = TwitchIRCClient::<SecureTCPTransport, _>::new(config);

        // Store the client in self
        self.client = Some(Arc::new(client));

        // Create an unbounded channel for parsed message events
        let (tx, rx) = unbounded_channel();
        self.tx = Some(tx);
        self.rx = Some(rx);

        // Start a background task reading from `incoming_messages`:
        let tx_for_task = self.tx.clone();
        let read_handle = tokio::spawn(async move {
            while let Some(msg) = incoming_messages.recv().await {
                match msg {
                    ServerMessage::Privmsg(privmsg) => {
                        let evt = TwitchIrcMessageEvent {
                            channel: privmsg.channel_login,
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
        self.read_task_handle = Some(read_handle);

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
        // Optionally do more cleanup
        Ok(())
    }

    async fn send_message(&self, channel: &str, message: &str) -> Result<(), Error> {
        // If you want to do `.say(channel, message)`, you need the actual client:
        if let Some(ref client) = self.client {
            // The library’s `join(...)` is for channel joining.
            // The method to send chat is `.say(channel, message).await`.
            client.say(channel.to_owned(), message.to_owned()).unwrap_or_else(|err| {
                error!("Error sending message to Twitch IRC: {:?}", err);
            });
        }
        Ok(())
    }

    async fn get_connection_status(&self) -> Result<crate::platforms::ConnectionStatus, Error> {
        Ok(self.connection_status.clone())
    }
}

//
// *******  HERE IS THE KEY PART: ChatPlatform for join_channel/leave_channel  *******
//
#[async_trait]
impl crate::platforms::ChatPlatform for TwitchIrcPlatform {
    async fn join_channel(&self, channel: &str) -> Result<(), Error> {
        if let Some(ref client) = self.client {
            client.join(channel.to_owned());
            Ok(())
        } else {
            Err(Error::Platform("No IRC client found in TwitchIrcPlatform".to_string()))
        }
    }

    async fn leave_channel(&self, channel: &str) -> Result<(), Error> {
        if let Some(ref client) = self.client {
            client.part(channel.to_owned());
            Ok(())
        } else {
            Err(Error::Platform("No IRC client found in TwitchIrcPlatform".to_string()))
        }
    }

    async fn get_channel_users(&self, _channel: &str) -> Result<Vec<String>, Error> {
        // The twitch-irc crate doesn’t natively provide a "who’s in channel" list,
        // so you might implement something custom or just return empty.
        Ok(Vec::new())
    }
}
