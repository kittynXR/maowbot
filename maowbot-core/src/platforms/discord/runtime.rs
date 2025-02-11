use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
use tokio::task::JoinHandle;
use tracing::{debug, error, info, warn};

use twilight_gateway::{self as gateway, CloseFrame, Config, EventTypeFlags, Intents, Shard, MessageSender, StreamExt};
use twilight_gateway::create_recommended;
use twilight_http::{Client as HttpClient};
use twilight_http::client::ClientBuilder;
use twilight_model::{
    gateway::event::Event,
    gateway::payload::incoming::MessageCreate,
    id::Id,
};

use crate::Error;
use crate::platforms::{ConnectionStatus, PlatformAuth, PlatformIntegration};

/// A simple struct holding minimal data from Discord messages.
#[derive(Debug, Clone)]
pub struct DiscordMessageEvent {
    pub channel: String,
    pub user_id: String,
    pub username: String,
    pub text: String,
}

pub struct DiscordPlatform {
    token: String,
    connection_status: ConnectionStatus,

    /// Our unbounded receiver for newly-arrived Discord messages.
    rx: Option<UnboundedReceiver<DiscordMessageEvent>>,

    /// The list of shard tasks (one task per shard).
    shard_tasks: Vec<JoinHandle<()>>,

    /// Keep track of each shard’s "sender" so we can close them on shutdown.
    shard_senders: Vec<MessageSender>,

    /// An HTTP client for sending messages, etc.
    http: Option<Arc<HttpClient>>,
}

impl DiscordPlatform {
    /// Creates a new DiscordPlatform. The token should be the raw bot token.
    pub fn new(token: String) -> Self {
        // Mask the token in logs.
        let masked = if token.len() >= 6 {
            let last6 = &token[token.len().saturating_sub(6)..];
            format!("(startsWith=****, endsWith={})", last6)
        } else {
            "(tooShortToMask)".to_string()
        };

        info!("DiscordPlatform::new called with token={}", masked);
        Self {
            token,
            connection_status: ConnectionStatus::Disconnected,
            rx: None,
            shard_tasks: Vec::new(),
            shard_senders: Vec::new(),
            http: None,
        }
    }

    /// Returns the next DiscordMessageEvent from the internal channel.
    pub async fn next_message_event(&mut self) -> Option<DiscordMessageEvent> {
        if let Some(rx) = &mut self.rx {
            rx.recv().await
        } else {
            None
        }
    }
}

/// Runs a shard’s event loop, sending received user messages via `tx`.
async fn shard_runner(mut shard: Shard, tx: UnboundedSender<DiscordMessageEvent>) {
    let shard_id = shard.id().number();
    info!("(ShardRunner) Shard {} started. Listening for events.", shard_id);

    while let Some(event_res) = shard.next_event(EventTypeFlags::all()).await {
        match event_res {
            Ok(event) => match event {
                Event::Ready(ready) => {
                    info!(
                        "(ShardRunner) Shard {} => READY as {}#{}",
                        shard_id,
                        ready.user.name,
                        ready.user.discriminator
                    );
                }
                Event::MessageCreate(msg) => {
                    // Unbox the message.
                    let msg: MessageCreate = *msg;
                    if msg.author.bot {
                        debug!(
                            "(ShardRunner) Shard {} => Ignoring message from bot {}",
                            shard_id, msg.author.name
                        );
                        continue;
                    }
                    let channel_str = msg.channel_id.to_string();
                    let user_id = msg.author.id.to_string();
                    let username = msg.author.name.clone();
                    let text = msg.content.clone();

                    debug!(
                        "(ShardRunner) Shard {} => Received message '{}' from {}",
                        shard_id, text, username
                    );
                    let _ = tx.send(DiscordMessageEvent {
                        channel: channel_str,
                        user_id,
                        username,
                        text,
                    });
                }
                Event::GuildCreate(gc) => {
                    let guild_create = *gc; // unbox
                    // In Twilight 0.16, GuildCreate has methods (or fields) for id and name.
                    // Depending on your exact version, you might have:
                    //   guild_create.name and guild_create.id
                    // or getters like guild_create.id()
                    // Here we try calling the getter:
                    info!(
                        "(ShardRunner) Shard {} => Joined guild: {} (id={})",
                        shard_id,
                        guild_create.id().to_string(),
                        guild_create.id().to_string()
                    );
                }
                other => {
                    debug!("(ShardRunner) Shard {} => Unhandled event: {:?}", shard_id, other);
                }
            },
            Err(err) => {
                error!(
                    "(ShardRunner) Shard {} => Error receiving event: {:?}",
                    shard_id, err
                );
            }
        }
    }
    warn!("(ShardRunner) Shard {} event loop ended.", shard_id);
}

#[async_trait]
impl PlatformAuth for DiscordPlatform {
    async fn authenticate(&mut self) -> Result<(), Error> {
        if self.token.is_empty() {
            return Err(Error::Auth("Empty Discord token".into()));
        }
        debug!("(DiscordPlatform) authenticate => Token is non-empty.");
        Ok(())
    }

    async fn refresh_auth(&mut self) -> Result<(), Error> {
        Ok(())
    }

    async fn revoke_auth(&mut self) -> Result<(), Error> {
        Ok(())
    }

    async fn is_authenticated(&self) -> Result<bool, Error> {
        Ok(!self.token.is_empty())
    }
}

#[async_trait]
impl PlatformIntegration for DiscordPlatform {
    async fn connect(&mut self) -> Result<(), Error> {
        if matches!(self.connection_status, ConnectionStatus::Connected) {
            info!("(DiscordPlatform) connect => Already connected; skipping.");
            return Ok(());
        }

        info!("(DiscordPlatform) connect => Starting Discord shards...");

        // Create an unbounded channel for our custom message events.
        let (tx, rx) = unbounded_channel::<DiscordMessageEvent>();
        self.rx = Some(rx);

        // Build a custom Twilight HTTP client with an increased timeout.
        debug!("(DiscordPlatform) Creating HTTP client with 30s timeout...");
        let http = Arc::new(
            ClientBuilder::new()
                .token(self.token.clone())
                .timeout(Duration::from_secs(30))
                .build(),
        );
        self.http = Some(http.clone());

        // Build a config for the shards.
        let config = gateway::Config::new(
            self.token.clone(),
            Intents::GUILDS | Intents::GUILD_MESSAGES | Intents::MESSAGE_CONTENT,
        );
        debug!(
            "(DiscordPlatform) Built config for shards. Intents: GUILDS | GUILD_MESSAGES | MESSAGE_CONTENT"
        );

        // Create recommended shards.
        let shards = create_recommended(&http, config, |shard_id, builder| {
            debug!("(DiscordPlatform) Creating shard {}...", shard_id.number());
            builder.build()
        })
            .await
            .map_err(|e| Error::Platform(format!("Error creating recommended shards: {e:?}")))?;

        let shard_count = shards.len();
        info!(
            "(DiscordPlatform) create_recommended created {} shard(s).",
            shard_count
        );
        if shard_count == 0 {
            warn!("(DiscordPlatform) => No shards were created. The bot won't connect.");
        }

        // Spawn each shard.
        for shard in shards {
            let shard_id = shard.id().number();
            debug!("(DiscordPlatform) Spawning shard {}...", shard_id);
            self.shard_senders.push(shard.sender());
            let tx_for_shard = tx.clone();
            let handle = tokio::spawn(async move {
                shard_runner(shard, tx_for_shard).await;
            });
            self.shard_tasks.push(handle);
        }

        self.connection_status = ConnectionStatus::Connected;
        info!("(DiscordPlatform) connect => All shards spawned. Bot should now attempt to connect.");
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<(), Error> {
        info!("(DiscordPlatform) disconnect => Shutting down shards...");
        self.connection_status = ConnectionStatus::Disconnected;
        for (i, sender) in self.shard_senders.iter().enumerate() {
            debug!("(DiscordPlatform) Closing shard #{}...", i);
            let _ = sender.close(CloseFrame::NORMAL);
        }
        for (i, task) in self.shard_tasks.iter_mut().enumerate() {
            debug!("(DiscordPlatform) Waiting for shard #{} to finish...", i);
            let _ = task.await;
        }
        self.shard_tasks.clear();
        self.shard_senders.clear();
        info!("(DiscordPlatform) disconnect => All shards closed.");
        Ok(())
    }

    async fn send_message(&self, channel: &str, message: &str) -> Result<(), Error> {
        let channel_id_u64: u64 = channel.parse()
            .map_err(|_| Error::Platform(format!("Invalid channel ID '{channel}'")))?;
        let channel_id = Id::new(channel_id_u64);

        if let Some(http) = &self.http {
            debug!(
                "(DiscordPlatform) send_message => Sending message to channel {}: {}",
                channel_id, message
            );
            let create = http.create_message(channel_id).content(message);
            create
                .await
                .map_err(|err| Error::Platform(format!("Error sending message: {err:?}")))?;
        } else {
            warn!("(DiscordPlatform) send_message => No HTTP client available!");
        }
        Ok(())
    }

    async fn get_connection_status(&self) -> Result<ConnectionStatus, Error> {
        debug!(
            "(DiscordPlatform) get_connection_status => {:?}",
            self.connection_status
        );
        Ok(self.connection_status.clone())
    }
}
