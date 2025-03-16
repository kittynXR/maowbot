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
use twilight_http::{client::ClientBuilder, Client as HttpClient};
use twilight_model::channel::ChannelType;
use twilight_model::gateway::payload::incoming::{MessageCreate, Ready as ReadyPayload};
use twilight_model::id::marker::{ChannelMarker, GuildMarker};
use twilight_model::id::Id;

use crate::Error;
use crate::eventbus::EventBus;
use maowbot_common::error::Error as CommonError;
use maowbot_common::models::discord::{DiscordAccountRecord, DiscordChannelRecord, DiscordGuildRecord};
use maowbot_common::traits::api::DiscordApi;
use maowbot_common::traits::platform_traits::{ConnectionStatus, PlatformAuth, PlatformIntegration};

/// Holds incoming Discord chat messages from the shard task.
#[derive(Debug, Clone)]
pub struct DiscordMessageEvent {
    pub channel: String,
    pub user_id: String,
    pub username: String,
    pub text: String,
    pub user_roles: Vec<String>,
}

/// The shard loop. It reads Gateway events, updates the cache, and forwards
/// message-create events to `tx`.
pub async fn shard_runner(
    mut shard: Shard,
    tx: UnboundedSender<DiscordMessageEvent>,
    http: Arc<HttpClient>,
    event_bus: Option<Arc<EventBus>>,
    cache: Arc<InMemoryCache>,
) {
    let shard_id = shard.id().number();
    info!("(ShardRunner) Shard {} started. Listening for events.", shard_id);

    while let Some(event_res) = shard.next_event(EventTypeFlags::all()).await {
        match event_res {
            Ok(event) => {
                // Update our cache with every event.
                cache.update(&event);

                match &event {
                    Event::Ready(ready_box) => {
                        let ready_data: &ReadyPayload = ready_box.as_ref();
                        let user = &ready_data.user;
                        info!(
                            "(ShardRunner) Shard {} => READY as {}#{} (ID={})",
                            shard_id, user.name, user.discriminator, user.id
                        );
                    }
                    Event::MessageCreate(msg_create) => {
                        let msg: &MessageCreate = msg_create;
                        if msg.author.bot {
                            debug!("Ignoring bot message from {}", msg.author.name);
                            continue;
                        }

                        let channel_name = match http.channel(msg.channel_id).await {
                            Ok(response) => match response.model().await {
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
                            },
                            Err(e) => {
                                error!("Error fetching channel => {:?}", e);
                                msg.channel_id.to_string()
                            }
                        };

                        let user_id = msg.author.id.to_string();
                        let username = msg.author.name.clone();
                        let text = msg.content.clone();
                        let user_roles: Vec<String> = msg
                            .member
                            .as_ref()
                            .map(|m| m.roles.iter().map(|r| r.to_string()).collect())
                            .unwrap_or_default();

                        debug!(
                            "(ShardRunner) Shard {} => msg '{}' from {} in '{}'; roles={:?}",
                            shard_id, text, username, channel_name, user_roles
                        );

                        let _ = tx.send(DiscordMessageEvent {
                            channel: channel_name,
                            user_id,
                            username,
                            text,
                            user_roles,
                        });

                        if let Some(bus) = &event_bus {
                            // Optionally broadcast to your EventBus
                            trace!("(EventBus) Not implemented for Discord chat yet.");
                            let _ = bus;
                        }
                    }
                    _ => {
                        trace!(
                            "(ShardRunner) Shard {} => unhandled event: {:?}",
                            shard_id,
                            event
                        );
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

/// DiscordPlatform is our ephemeral Discord client + runtime state.
pub struct DiscordPlatform {
    pub token: String,
    pub connection_status: ConnectionStatus,

    // Where incoming messages are queued.
    pub rx: Option<UnboundedReceiver<DiscordMessageEvent>>,
    pub shard_tasks: Vec<JoinHandle<()>>,
    pub shard_senders: Vec<MessageSender>,

    pub(crate) http: Option<Arc<HttpClient>>,
    pub cache: Option<Arc<InMemoryCache>>,
    pub event_bus: Option<Arc<EventBus>>,
}

impl DiscordPlatform {
    pub fn new(token: String) -> Self {
        info!("DiscordPlatform::new token=(masked)");
        Self {
            token,
            connection_status: ConnectionStatus::Disconnected,
            rx: None,
            shard_tasks: Vec::new(),
            shard_senders: Vec::new(),
            http: None,
            cache: None,
            event_bus: None,
        }
    }

    pub fn set_event_bus(&mut self, bus: Arc<EventBus>) {
        self.event_bus = Some(bus);
    }

    /// Wait for the next inbound Discord message (if any).
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
        // Discord bot tokens generally do not expire.
        Ok(())
    }

    async fn revoke_auth(&mut self) -> Result<(), Error> {
        // No direct revoke path for Discord bots
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

        let config = Config::new(
            self.token.clone(),
            Intents::GUILDS | Intents::GUILD_MESSAGES | Intents::MESSAGE_CONTENT,
        );

        let shards = gateway::create_recommended(&http_client, config, |_, b| b.build())
            .await
            .map_err(|e| Error::Platform(format!("Error creating recommended shards: {e:?}")))?;

        let shard_count = shards.len();
        info!(
            "(DiscordPlatform) create_recommended => {} shard(s).",
            shard_count
        );

        for shard in shards {
            self.shard_senders.push(shard.sender());

            let tx_for_shard = tx.clone();
            let http_for_shard = http_client.clone();
            let bus_for_shard = self.event_bus.clone();
            let cache_for_shard = cache.clone();

            let handle = tokio::spawn(async move {
                shard_runner(
                    shard,
                    tx_for_shard,
                    http_for_shard,
                    bus_for_shard,
                    cache_for_shard,
                )
                    .await;
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

    /// Send a text message by:
    /// 1. Setting the `.content(...)`
    /// 2. `.await` the returned future
    async fn send_message(&self, channel: &str, message: &str) -> Result<(), Error> {
        let channel_id_u64: u64 = channel.parse().map_err(|_| {
            Error::Platform(format!("Invalid channel ID '{channel}' (must be numeric)"))
        })?;
        let channel_id = Id::<ChannelMarker>::new(channel_id_u64);

        if let Some(http) = &self.http {
            // Build the request
            let fut = http.create_message(channel_id).content(message);

            // Now `.content(...)` returns a `CreateMessage<'_>`, which is a Future.
            // The actual sending + validation + HTTP call happens at `.await`.
            fut.await.map_err(|err| {
                Error::Platform(format!("Error sending Discord message: {err:?}"))
            })?;
        } else {
            warn!("(DiscordPlatform) send_message => no HttpClient available?");
        }

        Ok(())
    }

    async fn get_connection_status(&self) -> Result<ConnectionStatus, Error> {
        Ok(self.connection_status.clone())
    }
}

/// Ephemeral `DiscordApi` implementation, suitable for a single-bot use case
/// or as a placeholder if you do not store data persistently in a DB.
#[async_trait]
impl DiscordApi for DiscordPlatform {
    async fn list_discord_guilds(
        &self,
        account_name: &str,
    ) -> Result<Vec<DiscordGuildRecord>, CommonError> {
        let cache = match &self.cache {
            Some(c) => c,
            None => return Ok(vec![]),
        };

        let mut out = Vec::new();
        for guild_ref in cache.iter().guilds() {
            let guild_id = guild_ref.key();
            let guild = guild_ref.value();
            let name = guild.name().to_string(); // `CachedGuild::name()` is &str in 0.16

            out.push(DiscordGuildRecord {
                account_name: account_name.to_string(),
                guild_id: guild_id.to_string(),
                guild_name: name,
                is_active: false,
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
            });
        }

        Ok(out)
    }

    async fn list_discord_channels(
        &self,
        account_name: &str,
        guild_id_str: &str,
    ) -> Result<Vec<DiscordChannelRecord>, CommonError> {
        let cache = match &self.cache {
            Some(c) => c,
            None => return Ok(vec![]),
        };

        let guild_id_u64 = guild_id_str.parse::<u64>().map_err(|_| {
            CommonError::Platform(format!("Guild ID '{guild_id_str}' not numeric"))
        })?;
        let guild_id = Id::<GuildMarker>::new(guild_id_u64);

        let mut out = Vec::new();
        for channel_ref in cache.iter().channels() {
            let channel_id = channel_ref.key();
            let channel = channel_ref.value();

            if channel.guild_id == Some(guild_id) {
                let ch_name = channel
                    .name
                    .clone()
                    .unwrap_or_else(|| channel_id.to_string());

                out.push(DiscordChannelRecord {
                    account_name: account_name.to_string(),
                    guild_id: guild_id_str.to_string(),
                    channel_id: channel_id.to_string(),
                    channel_name: ch_name,
                    is_active: false,
                    created_at: chrono::Utc::now(),
                    updated_at: chrono::Utc::now(),
                });
            }
        }

        Ok(out)
    }

    #[cfg(feature = "interactions")]
    async fn list_discord_commands(
        &self,
        account_name: &str,
    ) -> Result<Vec<(String, String)>, CommonError> {
        use twilight_model::id::marker::ApplicationMarker;

        if self.http.is_none() {
            return Err(CommonError::Platform("No HTTP client".into()));
        }
        let http = self.http.as_ref().unwrap();

        let app_resp = http
            .current_user_application()
            .await
            .map_err(|e| CommonError::Platform(format!("Discord get application error => {e:?}")))?;
        let app = app_resp
            .model()
            .await
            .map_err(|e| CommonError::Platform(format!("Discord parse application => {e:?}")))?;
        let application_id = app.id;

        let commands_response = http
            .interaction(application_id)
            .global_commands()
            .await
            .map_err(|e| CommonError::Platform(format!("Discord fetch commands => {e:?}")))?;

        let commands = commands_response
            .models()
            .await
            .map_err(|e| CommonError::Platform(format!("Discord parse commands => {e:?}")))?;

        info!(
            "(list_discord_commands) Found {} global command(s) for ephemeral account '{}'.",
            commands.len(),
            account_name
        );

        let mut out = Vec::new();
        for cmd in commands {
            out.push((cmd.id.to_string(), cmd.name.clone()));
        }
        Ok(out)
    }

    #[cfg(not(feature = "interactions"))]
    async fn list_discord_commands(
        &self,
        _account_name: &str,
    ) -> Result<Vec<(String, String)>, CommonError> {
        // If you're not using slash commands, just return empty
        Ok(vec![])
    }

    async fn send_discord_message(
        &self,
        _account_name: &str,
        _guild_id: &str,
        channel_id: &str,
        text: &str
    ) -> Result<(), CommonError> {
        self.send_message(channel_id, text).await?;
        Ok(())
    }
}
