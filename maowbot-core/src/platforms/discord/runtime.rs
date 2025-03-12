// maowbot-core/src/platforms/discord/runtime.rs

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
use tokio::task::JoinHandle;
use tracing::{debug, error, info, trace, warn};

use twilight_gateway::{
    self as gateway,
    CloseFrame,
    Config,
    EventTypeFlags,
    Intents,
    Shard,
    MessageSender,
    StreamExt
};
use twilight_http::{Client as HttpClient};
use twilight_http::client::ClientBuilder;
use twilight_model::channel::{Channel as DiscordChannel, ChannelType};
use twilight_model::{
    gateway::event::Event,
    gateway::payload::incoming::MessageCreate,
    guild::Member,
    id::Id,
};

use crate::Error;
use crate::eventbus::EventBus;
use maowbot_common::traits::platform_traits::{ConnectionStatus, PlatformAuth, PlatformIntegration};

/// A simple struct holding minimal data from Discord messages.
/// (We keep this for demonstration, but now we also directly publish to `EventBus`.)
#[derive(Debug, Clone)]
pub struct DiscordMessageEvent {
    pub channel: String,
    pub user_id: String,
    pub username: String,
    pub text: String,
    /// Optionally store "roles" from the guild member if present.
    /// In Discord, these are role IDs, but we store them as strings.
    pub user_roles: Vec<String>,
}

/// Runs a shard’s event loop, sending user messages to `tx` and also publishing
/// them to the event bus for our message service to store in the DB.
pub async fn shard_runner(
    mut shard: Shard,
    tx: UnboundedSender<DiscordMessageEvent>,
    http: Arc<HttpClient>,
    event_bus: Option<Arc<EventBus>>,
) {
    let shard_id = shard.id().number();
    info!("(ShardRunner) Shard {} started. Listening for events.", shard_id);

    while let Some(event_res) = shard.next_event(EventTypeFlags::all()).await {
        match event_res {
            Ok(event) => match event {
                // Example: "Ready" event
                Event::Ready(ready) => {
                    info!(
                        "(ShardRunner) Shard {} => READY as {}#{}",
                        shard_id, ready.user.name, ready.user.discriminator
                    );
                }

                // MessageCreate events
                Event::MessageCreate(msg) => {
                    let msg: MessageCreate = *msg; // unbox
                    if msg.author.bot {
                        debug!(
                            "(ShardRunner) ignoring message from bot {}",
                            msg.author.name
                        );
                        continue;
                    }

                    // Attempt to retrieve the channel name/kind:
                    let channel_name = match http.channel(msg.channel_id).await {
                        Ok(response) => {
                            match response.model().await {
                                Ok(channel_obj) => match channel_obj.kind {
                                    ChannelType::GuildText
                                    | ChannelType::GuildVoice
                                    | ChannelType::GuildForum
                                    | ChannelType::GuildStageVoice => {
                                        channel_obj
                                            .name
                                            .unwrap_or_else(|| msg.channel_id.to_string())
                                    }
                                    ChannelType::Private => {
                                        // DM channel
                                        format!("(DM {})", msg.channel_id)
                                    }
                                    ChannelType::Group => {
                                        // Group DM
                                        "(Group DM)".to_string()
                                    }
                                    _ => msg.channel_id.to_string(),
                                },
                                Err(e) => {
                                    error!("Error parsing channel model => {:?}", e);
                                    msg.channel_id.to_string()
                                }
                            }
                        }
                        Err(e) => {
                            error!("Error fetching channel => {:?}", e);
                            msg.channel_id.to_string()
                        }
                    };

                    let user_id = msg.author.id.to_string();
                    let username = msg.author.name.clone();
                    let text = msg.content.clone();

                    // Attempt to read roles from the "member" object if present:
                    let user_roles: Vec<String> = match &msg.member {
                        Some(mem) => mem.roles.iter().map(|rid| rid.to_string()).collect(),
                        None => vec![],
                    };

                    debug!(
                        "(ShardRunner) Shard {} => msg '{}' from {} in '{}'; roles={:?}",
                        shard_id, text, username, channel_name, user_roles
                    );

                    // Send to our local unbounded channel (if some other part wants it).
                    let _ = tx.send(DiscordMessageEvent {
                        channel: channel_name.clone(),
                        user_id: user_id.clone(),
                        username: username.clone(),
                        text: text.clone(),
                        user_roles: user_roles.clone(),
                    });

                    // Also publish a ChatMessage to the event bus:
                    if let Some(_bus) = &event_bus {
                        // We'll embed roles the same way TwitchIRC does: "user_id|roles=r1,r2"
                        let joined_roles = if !user_roles.is_empty() {
                            format!("|roles={}", user_roles.join(","))
                        } else {
                            "".to_string()
                        };
                        let _combined_user_str = format!("{}{}", user_id, joined_roles);

                        // bus.publish_chat("discord", &channel_name, &combined_user_str, &text).await;
                    }
                }

                // Example: joined a guild
                Event::GuildCreate(gc) => {
                    let g = *gc; // unbox
                    info!(
                        "(ShardRunner) Shard {} => joined guild id={}",
                        shard_id, g.id()
                    );
                }

                // Fallback
                other => {
                    trace!("(ShardRunner) Shard {} => Unhandled event: {:?}", shard_id, other);
                }
            },
            Err(err) => {
                error!("(ShardRunner) Shard {} => error receiving event: {:?}", shard_id, err);
            }
        }
    }

    warn!("(ShardRunner) Shard {} event loop ended.", shard_id);
}

/// Holds the Discord connection (shards, tasks, etc.).
pub struct DiscordPlatform {
    token: String,
    connection_status: ConnectionStatus,

    /// Our unbounded receiver for newly-arrived Discord messages (if you want to read them).
    rx: Option<UnboundedReceiver<DiscordMessageEvent>>,

    /// The list of shard tasks (one task per shard).
    shard_tasks: Vec<JoinHandle<()>>,

    /// Keep track of each shard’s "sender" so we can close them on shutdown.
    shard_senders: Vec<MessageSender>,

    /// An HTTP client for sending messages and fetching channel names.
    pub(crate) http: Option<Arc<HttpClient>>,

    /// An Arc to the EventBus so we can publish messages for DB, etc.
    pub event_bus: Option<Arc<EventBus>>,
}

impl DiscordPlatform {
    /// Creates a new DiscordPlatform. The token should be the raw bot token.
    pub fn new(token: String) -> Self {
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
            event_bus: None,
        }
    }

    /// If you want to store a reference to the EventBus, call this before connect().
    pub fn set_event_bus(&mut self, bus: Arc<EventBus>) {
        self.event_bus = Some(bus);
    }

    /// Returns the next DiscordMessageEvent from the internal channel.
    /// Not strictly needed if we're just using EventBus to handle messages.
    pub async fn next_message_event(&mut self) -> Option<DiscordMessageEvent> {
        if let Some(rx) = &mut self.rx {
            rx.recv().await
        } else {
            None
        }
    }
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

        info!("(DiscordPlatform) connect => starting Discord shards...");

        // Create an unbounded channel for our custom message events:
        let (tx, rx) = unbounded_channel::<DiscordMessageEvent>();
        self.rx = Some(rx);

        // Build a Twilight HTTP client with an increased timeout.
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
            "(DiscordPlatform) built config for shards => Intents: GUILDS | GUILD_MESSAGES | MESSAGE_CONTENT"
        );

        // Create recommended shards:
        let shards = gateway::create_recommended(&http, config, |shard_id, builder| {
            debug!("(DiscordPlatform) creating shard {}...", shard_id.number());
            builder.build()
        })
            .await
            .map_err(|e| Error::Platform(format!("Error creating recommended shards: {e:?}")))?;

        let shard_count = shards.len();
        info!(
            "(DiscordPlatform) create_recommended => {} shard(s).",
            shard_count
        );
        if shard_count == 0 {
            warn!("(DiscordPlatform) => no shards were created. The bot won't connect.");
        }

        // Spawn each shard
        for shard in shards {
            let shard_id = shard.id().number();
            debug!("(DiscordPlatform) spawning shard {}...", shard_id);
            self.shard_senders.push(shard.sender());
            let tx_for_shard = tx.clone();
            let http_for_shard = http.clone();
            let bus_for_shard = self.event_bus.clone();
            let handle = tokio::spawn(async move {
                shard_runner(shard, tx_for_shard, http_for_shard, bus_for_shard).await;
            });
            self.shard_tasks.push(handle);
        }

        self.connection_status = ConnectionStatus::Connected;
        info!("(DiscordPlatform) connect => all shards spawned.");
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<(), Error> {
        info!("(DiscordPlatform) disconnect => shutting down shards...");
        self.connection_status = ConnectionStatus::Disconnected;
        for (i, sender) in self.shard_senders.iter().enumerate() {
            debug!("(DiscordPlatform) closing shard #{}...", i);
            let _ = sender.close(CloseFrame::NORMAL);
        }
        for (i, task) in self.shard_tasks.iter_mut().enumerate() {
            debug!("(DiscordPlatform) waiting for shard #{} to finish...", i);
            let _ = task.await;
        }
        self.shard_tasks.clear();
        self.shard_senders.clear();
        info!("(DiscordPlatform) disconnect => all shards closed.");
        Ok(())
    }

    async fn send_message(&self, channel: &str, message: &str) -> Result<(), Error> {
        let channel_id_u64: u64 = channel.parse().map_err(|_| {
            Error::Platform(format!("Invalid channel ID or name '{channel}' (parsing as u64)"))
        })?;
        let channel_id = Id::new(channel_id_u64);

        if let Some(http) = &self.http {
            debug!(
                "(DiscordPlatform) send_message => sending to channel {}: {}",
                channel_id, message
            );
            let create = http.create_message(channel_id).content(message);
            create
                .await
                .map_err(|err| Error::Platform(format!("Error sending message: {err:?}")))?;
        } else {
            warn!("(DiscordPlatform) send_message => no HTTP client available!");
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