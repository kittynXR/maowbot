// maowbot-core/src/platforms/discord/runtime.rs

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
use tokio::task::JoinHandle;
use tracing::{debug, error, info, trace, warn};

use twilight_cache_inmemory::{InMemoryCache, ResourceType};
use twilight_gateway::{
    self as gateway,
    CloseFrame,
    Config,
    Event,
    EventTypeFlags,
    Intents,
    Shard,
    MessageSender,
    StreamExt,
};
use twilight_http::client::ClientBuilder;
use twilight_http::Client as HttpClient;
use twilight_model::channel::{Channel as DiscordChannel, ChannelType};
use twilight_model::gateway::payload::incoming::{
    ChannelCreate, GuildCreate, MessageCreate, Ready as ReadyPayload,
};
use twilight_model::id::Id;

use crate::Error;
use crate::eventbus::EventBus;
use maowbot_common::traits::platform_traits::{ConnectionStatus, PlatformAuth, PlatformIntegration};
use maowbot_common::traits::repository_traits::DiscordRepository;

/// Minimal struct carrying data from Discord messages for local usage.
#[derive(Debug, Clone)]
pub struct DiscordMessageEvent {
    pub channel: String,
    pub user_id: String,
    pub username: String,
    pub text: String,
    pub user_roles: Vec<String>,
}

/// Main shard task: processes Discord gateway events, updates the in-memory cache,
/// sends minimal messages to `tx`, and optionally updates the DB via `DiscordRepository`.
pub async fn shard_runner(
    mut shard: Shard,
    tx: UnboundedSender<DiscordMessageEvent>,
    http: Arc<HttpClient>,
    event_bus: Option<Arc<EventBus>>,
    cache: Arc<InMemoryCache>,
    discord_repo: Option<Arc<dyn DiscordRepository + Send + Sync>>,
    account_name: String, // which Discord bot account we're using
) {
    let shard_id = shard.id().number();
    info!("(ShardRunner) Shard {} started. Listening for events.", shard_id);

    while let Some(event_res) = shard.next_event(EventTypeFlags::all()).await {
        match event_res {
            Ok(event) => {
                // Always update the in-memory cache:
                cache.update(&event);

                match &event {
                    Event::Ready(ready_box) => {
                        // ready_box is Box<ReadyPayload>, so we can deref it:
                        let ready_data: &ReadyPayload = ready_box.as_ref();
                        let user = &ready_data.user;
                        info!(
                            "(ShardRunner) Shard {} => READY as {}#{} (ID={})",
                            shard_id, user.name, user.discriminator, user.id
                        );

                        // Optionally, after we see READY, we could do a manual REST fetch to ensure
                        // all guilds/channels are in the DB. See prior explanation if needed.
                    }

                    Event::MessageCreate(msg_create) => {
                        let msg: &MessageCreate = msg_create;
                        if msg.author.bot {
                            debug!("(ShardRunner) ignoring bot message from {}", msg.author.name);
                            continue;
                        }

                        // Attempt to fetch the channel name from the HTTP client:
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
                                            format!("(DM {})", msg.channel_id)
                                        }
                                        ChannelType::Group => "(Group DM)".to_string(),
                                        _ => msg.channel_id.to_string(),
                                    },
                                    Err(e) => {
                                        error!("Error parsing channel => {:?}", e);
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
                        let user_roles: Vec<String> = match &msg.member {
                            Some(mem) => mem.roles.iter().map(|rid| rid.to_string()).collect(),
                            None => vec![],
                        };

                        debug!(
                            "(ShardRunner) Shard {} => msg '{}' from {} in '{}'; roles={:?}",
                            shard_id, text, username, channel_name, user_roles
                        );

                        // Send a local copy to the unbounded channel:
                        let _ = tx.send(DiscordMessageEvent {
                            channel: channel_name.clone(),
                            user_id: user_id.clone(),
                            username: username.clone(),
                            text: text.clone(),
                            user_roles: user_roles.clone(),
                        });

                        // Optionally publish a chat event to the event bus:
                        if let Some(bus) = &event_bus {
                            // bus.publish_chat("discord", &channel_name, &user_id, &text).await;
                            trace!("(EventBus) Not implemented for Discord chat");
                        }
                    }

                    Event::GuildCreate(guild_create_event) => {
                        // We have two variants: Available(...) or Unavailable(...).
                        // We'll store them in the DB as needed.
                        if let Some(repo) = &discord_repo {
                            match &**guild_create_event {
                                GuildCreate::Available(g) => {
                                    let guild_str_id = g.id.to_string();
                                    let guild_name = &g.name;
                                    info!("(ShardRunner) GuildCreate => Found guild '{}' (ID={})", guild_name, guild_str_id);
                                    let _ = repo.upsert_guild(&account_name, &guild_str_id, guild_name).await;

                                    // Also store channels we already know from the event:
                                    for ch in &g.channels {
                                        let ch_id = ch.id.to_string();
                                        let ch_name = ch
                                            .name
                                            .clone()
                                            .unwrap_or_else(|| ch_id.clone());
                                        let _ = repo
                                            .upsert_channel(
                                                &account_name,
                                                &guild_str_id,
                                                &ch_id,
                                                &ch_name,
                                            )
                                            .await;
                                    }
                                }
                                GuildCreate::Unavailable(u) => {
                                    let guild_str_id = u.id.to_string();
                                    warn!("(ShardRunner) GuildCreate => Unavailable guild ID={}", guild_str_id);
                                    let _ = repo
                                        .upsert_guild(&account_name, &guild_str_id, "[Unavailable]")
                                        .await;
                                }
                            }
                        }
                    }

                    Event::ChannelCreate(ch_create_event) => {
                        // "ChannelCreate" is a tuple struct with .0 public, so ch_create_event.0 is valid:
                        let ChannelCreate(ch) = &**ch_create_event;
                        if let Some(repo) = &discord_repo {
                            // If it's a guild channel, store it
                            if let Some(guild_id_val) = ch.guild_id {
                                let guild_str_id = guild_id_val.to_string();
                                let channel_str_id = ch.id.to_string();
                                let channel_name = ch
                                    .name
                                    .clone()
                                    .unwrap_or_else(|| channel_str_id.clone());
                                let _ = repo
                                    .upsert_channel(
                                        &account_name,
                                        &guild_str_id,
                                        &channel_str_id,
                                        &channel_name,
                                    )
                                    .await;
                            }
                        }
                    }

                    _ => {
                        trace!("(ShardRunner) Shard {} => unhandled event: {:?}", shard_id, event);
                    }
                }
            }
            Err(err) => {
                error!(
                    "(ShardRunner) Shard {} => error receiving event: {:?}",
                    shard_id, err
                );
            }
        }
    }

    warn!("(ShardRunner) Shard {} event loop ended.", shard_id);
}

/// Holds the Discord connection info, shards, tasks, etc.
pub struct DiscordPlatform {
    token: String,
    connection_status: ConnectionStatus,

    /// If you want to receive messages from Discord as `DiscordMessageEvent` in your application,
    /// you can poll `rx` from somewhere else. If unused, you can remove it.
    rx: Option<UnboundedReceiver<DiscordMessageEvent>>,

    /// Each shard runs in its own task; we keep their JoinHandles to manage them.
    shard_tasks: Vec<JoinHandle<()>>,

    /// Each shard has a `MessageSender` for graceful shutdown.
    shard_senders: Vec<MessageSender>,

    /// HTTP client for sending messages/fetching channel data.
    pub(crate) http: Option<Arc<HttpClient>>,

    /// Optional event bus reference.
    pub event_bus: Option<Arc<EventBus>>,

    /// In-memory cache from Twilight, updated by the shard runner.
    pub cache: Option<Arc<InMemoryCache>>,

    /// Our repository for persisting discovered guilds/channels, if configured.
    pub discord_repo: Option<Arc<dyn DiscordRepository + Send + Sync>>,

    /// Which account_name (from platform_credentials.user_name) we’re using.
    pub account_name: Option<String>,
}

impl DiscordPlatform {
    /// Creates a new DiscordPlatform struct. Nothing is connected until you call `connect()`.
    pub fn new(token: String) -> Self {
        info!("DiscordPlatform::new token=(masked)");
        Self {
            token,
            connection_status: ConnectionStatus::Disconnected,
            rx: None,
            shard_tasks: Vec::new(),
            shard_senders: Vec::new(),
            http: None,
            event_bus: None,
            cache: None,
            discord_repo: None,
            account_name: None,
        }
    }

    /// Optional setter if you want to store an `EventBus` reference for publishing events.
    pub fn set_event_bus(&mut self, bus: Arc<EventBus>) {
        self.event_bus = Some(bus);
    }

    /// Assigns the repository + account name for storing discovered guilds/channels in DB.
    pub fn set_discord_repository(
        &mut self,
        repo: Arc<dyn DiscordRepository + Send + Sync>,
        account_name: String
    ) {
        self.discord_repo = Some(repo);
        self.account_name = Some(account_name);
    }

    /// If you want to handle messages (DiscordMessageEvent) yourself, you can poll from `rx`.
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
        debug!("(DiscordPlatform) authenticate => token is non-empty.");
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
            info!("(DiscordPlatform) connect => already connected; skipping.");
            return Ok(());
        }

        info!("(DiscordPlatform) connect => starting Discord shards...");
        let (tx, rx) = unbounded_channel::<DiscordMessageEvent>();
        self.rx = Some(rx);

        let http_client = Arc::new(
            ClientBuilder::new()
                .token(self.token.clone())
                .timeout(Duration::from_secs(30))
                .build()
        );
        self.http = Some(http_client.clone());

        let cache = InMemoryCache::builder()
            .resource_types(ResourceType::GUILD | ResourceType::CHANNEL | ResourceType::MESSAGE)
            .build();
        let cache = Arc::new(cache);
        self.cache = Some(cache.clone());

        let config = gateway::Config::new(
            self.token.clone(),
            Intents::GUILDS | Intents::GUILD_MESSAGES | Intents::MESSAGE_CONTENT
        );

        let shards = gateway::create_recommended(&http_client, config, |shard_id, builder| {
            builder.build()
        })
            .await
            .map_err(|e| Error::Platform(format!("Error creating recommended shards: {e:?}")))?;

        let shard_count = shards.len();
        info!("(DiscordPlatform) create_recommended => {} shard(s).", shard_count);

        for shard in shards {
            let shard_id = shard.id().number();
            self.shard_senders.push(shard.sender());

            let tx_for_shard = tx.clone();
            let http_for_shard = http_client.clone();
            let bus_for_shard = self.event_bus.clone();
            let cache_for_shard = cache.clone();
            let repo_for_shard = self.discord_repo.clone();
            let acct_name = self.account_name.clone().unwrap_or_else(|| "UnknownAccount".into());

            let handle = tokio::spawn(async move {
                shard_runner(
                    shard,
                    tx_for_shard,
                    http_for_shard,
                    bus_for_shard,
                    cache_for_shard,
                    repo_for_shard,
                    acct_name
                ).await;
            });
            self.shard_tasks.push(handle);
        }

        self.connection_status = ConnectionStatus::Connected;
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
        info!("(DiscordPlatform) disconnected.");
        Ok(())
    }

    async fn send_message(&self, channel: &str, message: &str) -> Result<(), Error> {
        let channel_id_u64: u64 = channel.parse().map_err(|_| {
            Error::Platform(format!("Invalid channel ID '{channel}' (must be numeric)"))
        })?;
        let channel_id = Id::new(channel_id_u64);

        if let Some(http) = &self.http {
            let req = http.create_message(channel_id).content(message);
            req.await.map_err(|err| Error::Platform(format!("Error sending message: {err:?}")))?;
        } else {
            warn!("(DiscordPlatform) send_message => no HTTP client available?");
        }
        Ok(())
    }

    async fn get_connection_status(&self) -> Result<ConnectionStatus, Error> {
        Ok(self.connection_status.clone())
    }
}
