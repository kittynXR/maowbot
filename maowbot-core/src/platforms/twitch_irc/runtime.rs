use async_trait::async_trait;
use std::sync::Arc;
use futures_util::TryFutureExt;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
use tokio::task::JoinHandle;
use tracing::{error, info, warn, debug};

use twitch_irc::login::StaticLoginCredentials;
use twitch_irc::message::ServerMessage;
use twitch_irc::{ClientConfig, SecureTCPTransport, TwitchIRCClient};

use crate::Error;
use crate::eventbus::EventBus;
use crate::models::PlatformCredential;
use crate::platforms::{
    ChatPlatform, ConnectionStatus, PlatformAuth, PlatformIntegration,
};

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

    pub read_task_handle: Option<JoinHandle<()>>,

    pub rx: Option<UnboundedReceiver<TwitchIrcMessageEvent>>,
    pub tx: Option<UnboundedSender<TwitchIrcMessageEvent>>,

    pub client: Option<Arc<TwitchIRCClient<SecureTCPTransport, StaticLoginCredentials>>>,

    /// NEW: optional event bus for publishing chat messages (so TUI can see them).
    pub event_bus: Option<Arc<EventBus>>,
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
            event_bus: None,
        }
    }

    pub fn set_credentials(&mut self, creds: PlatformCredential) {
        self.credentials = Some(creds);
    }

    pub fn credentials(&self) -> Option<&PlatformCredential> {
        self.credentials.as_ref()
    }

    /// If your system uses an EventBus, you can set it here.
    pub fn set_event_bus(&mut self, bus: Arc<EventBus>) {
        self.event_bus = Some(bus);
    }

    pub async fn next_message_event(&mut self) -> Option<TwitchIrcMessageEvent> {
        if let Some(rx) = &mut self.rx {
            rx.recv().await
        } else {
            None
        }
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

        debug!(
            "TwitchIrcPlatform connecting user_name='{}', user_id={} is_bot={}",
            creds.user_name,
            creds.user_id,
            creds.is_bot
        );

        // Optionally validate token via /validate
        let raw_token = creds
            .primary_token
            .strip_prefix("oauth:")
            .unwrap_or(&creds.primary_token)
            .to_string();

        let user_login_name = creds.user_name.clone();

        let config = ClientConfig::new_simple(
            StaticLoginCredentials::new(user_login_name, Some(raw_token))
        );

        let (mut incoming_messages, client) = TwitchIRCClient::<SecureTCPTransport, _>::new(config);
        self.client = Some(Arc::new(client));

        let (tx, rx) = unbounded_channel();
        self.tx = Some(tx);
        self.rx = Some(rx);

        let tx_for_task = self.tx.clone();
        let bus_for_task = self.event_bus.clone();

        let read_handle = tokio::spawn(async move {
            info!("(TwitchIrcPlatform) starting IRC read loop...");
            while let Some(msg) = incoming_messages.recv().await {
                match msg {
                    ServerMessage::Privmsg(privmsg) => {
                        debug!("(TwitchIrcPlatform) PRIVMSG in #{} from {}: {}",
                               privmsg.channel_login,
                               privmsg.sender.login,
                               privmsg.message_text);

                        // 1) pass to our TUI MPSC
                        let evt = TwitchIrcMessageEvent {
                            channel: privmsg.channel_login.clone(),
                            user_name: privmsg.sender.login.clone(),
                            user_id: privmsg.sender.id.clone(),
                            text: privmsg.message_text.clone(),
                        };
                        if let Some(sender) = &tx_for_task {
                            let _ = sender.send(evt);
                        }

                        // 2) also publish to event bus so that
                        //    other parts of the system can see the chat message.
                        if let Some(ref bus) = bus_for_task {
                            bus.publish_chat(
                                "twitch-irc",
                                &privmsg.channel_login,
                                &privmsg.sender.login,
                                &privmsg.message_text
                            ).await;
                        }
                    }
                    other => {
                        debug!("(TwitchIrcPlatform) Non-PRIVMSG: {:?}", other);
                    }
                }
            }
            info!("(TwitchIrcPlatform) IRC read loop ended.");
        });
        self.read_task_handle = Some(read_handle);

        self.connection_status = ConnectionStatus::Connected;
        info!("(TwitchIrcPlatform) connected user_id={}", creds.user_id);
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
        if let Some(ref client) = self.client {
            debug!("(TwitchIrcPlatform) send_message => {}: {}", channel, message);
            client.say(channel.to_string(), message.to_string()).unwrap_or_else(|err| {
                error!("Error sending message to Twitch IRC: {:?}", err);
            });
        }
        Ok(())
    }

    async fn get_connection_status(&self) -> Result<ConnectionStatus, Error> {
        Ok(self.connection_status.clone())
    }
}

#[async_trait]
impl ChatPlatform for TwitchIrcPlatform {
    async fn join_channel(&self, channel: &str) -> Result<(), Error> {
        info!("(TwitchIrcPlatform) join_channel => '{}'", channel);
        if let Some(ref client) = self.client {
            client.join(channel.to_string());
            Ok(())
        } else {
            Err(Error::Platform("No IRC client found in TwitchIrcPlatform".to_string()))
        }
    }

    async fn leave_channel(&self, channel: &str) -> Result<(), Error> {
        info!("(TwitchIrcPlatform) leave_channel => '{}'", channel);
        if let Some(ref client) = self.client {
            client.part(channel.to_string());
            Ok(())
        } else {
            Err(Error::Platform("No IRC client found in TwitchIrcPlatform".to_string()))
        }
    }

    async fn get_channel_users(&self, _channel: &str) -> Result<Vec<String>, Error> {
        Ok(Vec::new())
    }
}
