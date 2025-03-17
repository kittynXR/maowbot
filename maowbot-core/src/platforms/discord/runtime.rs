use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
use tokio::sync::Mutex;
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
use twilight_http::Client as HttpClient;
use twilight_http::client::ClientBuilder;
use twilight_model::channel::ChannelType;
use twilight_model::gateway::payload::incoming::{MessageCreate, Ready as ReadyPayload};
use twilight_model::id::marker::{ChannelMarker, GuildMarker};
use twilight_model::id::Id;

use crate::Error;
use crate::eventbus::EventBus;
use maowbot_common::error::Error as CommonError;
use maowbot_common::models::discord::{DiscordChannelRecord, DiscordGuildRecord};
use maowbot_common::traits::api::DiscordApi;
use maowbot_common::traits::platform_traits::{ConnectionStatus, PlatformAuth, PlatformIntegration};

#[derive(Debug, Clone)]
pub struct DiscordMessageEvent {
    pub channel: String,
    pub user_id: String,
    pub username: String,
    pub text: String,
    pub user_roles: Vec<String>,
}

/// The shard runner function remains basically the same:
///   - calls `shard.next_event(...)`
///   - updates the in-memory cache
///   - sends inbound chat messages to `tx`.
async fn shard_runner(
    mut shard: Shard,
    tx: UnboundedSender<DiscordMessageEvent>,
    http: Arc<HttpClient>,
    event_bus: Option<Arc<EventBus>>,
    cache: Arc<InMemoryCache>,
) {
    let shard_id = shard.id().number();
    info!("(ShardRunner) Shard {shard_id} started. Listening for events.");

    while let Some(item) = shard.next_event(EventTypeFlags::all()).await {
        match item {
            Ok(event) => {
                cache.update(&event);

                match &event {
                    Event::Ready(ready) => {
                        let data: &ReadyPayload = ready.as_ref();
                        info!(
                            "Shard {shard_id} => READY as {}#{} (ID={})",
                            data.user.name, data.user.discriminator, data.user.id
                        );
                    }
                    Event::MessageCreate(msg_create) => {
                        let msg: &MessageCreate = msg_create;
                        if msg.author.bot {
                            debug!("Ignoring bot message from {}", msg.author.name);
                            continue;
                        }
                        let channel_name = match http.channel(msg.channel_id).await {
                            Ok(resp) => match resp.model().await {
                                Ok(ch_obj) => match ch_obj.kind {
                                    ChannelType::GuildText
                                    | ChannelType::GuildVoice
                                    | ChannelType::GuildForum
                                    | ChannelType::GuildStageVoice => {
                                        ch_obj.name.unwrap_or_else(|| msg.channel_id.to_string())
                                    }
                                    ChannelType::Private => format!("(DM {})", msg.channel_id),
                                    ChannelType::Group => "(Group DM)".to_string(),
                                    _ => msg.channel_id.to_string(),
                                },
                                Err(e) => {
                                    error!("Error parsing channel => {e:?}");
                                    msg.channel_id.to_string()
                                }
                            },
                            Err(e) => {
                                error!("Error fetching channel => {e:?}");
                                msg.channel_id.to_string()
                            }
                        };

                        let user_roles: Vec<String> = msg
                            .member
                            .as_ref()
                            .map(|m| m.roles.iter().map(|r| r.to_string()).collect())
                            .unwrap_or_default();

                        let _ = tx.send(DiscordMessageEvent {
                            channel: channel_name,
                            user_id: msg.author.id.to_string(),
                            username: msg.author.name.clone(),
                            text: msg.content.clone(),
                            user_roles,
                        });

                        if let Some(bus) = &event_bus {
                            trace!("(EventBus) Not implemented for Discord chat yet.");
                            let _ = bus;
                        }
                    }
                    _ => {
                        trace!("Shard {shard_id} => unhandled event: {event:?}");
                    }
                }
            }
            Err(err) => {
                error!("Shard {shard_id} => error receiving event: {err:?}");
            }
        }
    }

    warn!("(ShardRunner) Shard {shard_id} event loop ended.");
}

/// The main DiscordPlatform struct now stores:
///   - `rx: Mutex<Option<UnboundedReceiver<DiscordMessageEvent>>>`
pub struct DiscordPlatform {
    pub token: String,
    pub connection_status: ConnectionStatus,

    /// We store the receiver in an Option. By default in the constructor, it's None.
    pub rx: Mutex<Option<UnboundedReceiver<DiscordMessageEvent>>>,

    pub shard_tasks: Vec<JoinHandle<()>>,
    pub shard_senders: Vec<MessageSender>,

    pub http: Option<Arc<HttpClient>>,
    pub cache: Option<Arc<InMemoryCache>>,
    pub event_bus: Option<Arc<EventBus>>,
}

impl DiscordPlatform {
    pub fn new(token: String) -> Self {
        Self {
            token,
            connection_status: ConnectionStatus::Disconnected,
            // Start out with no channel set
            rx: Mutex::new(None),
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

    /// Callers can `await` the next inbound message. We'll lock `self.rx`,
    /// get the receiver from the Option, then call `.recv()` on it if present.
    pub async fn next_message_event(&self) -> Option<DiscordMessageEvent> {
        let mut guard = self.rx.lock().await;
        match guard.as_mut() {
            Some(r) => r.recv().await,
            None => None,
        }
    }
}

/// For auth, unchanged:
#[async_trait]
impl PlatformAuth for DiscordPlatform {
    async fn authenticate(&mut self) -> Result<(), Error> {
        if self.token.is_empty() {
            return Err(Error::Auth("Discord token is empty".into()));
        }
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

/// Connect, create the unbounded channel, store it in `rx`, and spawn the shard runner
#[async_trait]
impl PlatformIntegration for DiscordPlatform {
    async fn connect(&mut self) -> Result<(), Error> {
        if matches!(self.connection_status, ConnectionStatus::Connected) {
            info!("(DiscordPlatform) Already connected => skipping");
            return Ok(());
        }

        // Create the unbounded channel:
        let (tx, rx) = unbounded_channel::<DiscordMessageEvent>();

        // Store the receiver in our `Mutex<Option<Receiver<...>>>`
        {
            let mut guard = self.rx.lock().await;
            *guard = Some(rx);
        }

        // Prepare the Twilight client:
        let http_client = Arc::new(
            ClientBuilder::new()
                .token(self.token.clone())
                .timeout(Duration::from_secs(30))
                .build()
        );
        self.http = Some(http_client.clone());

        // Prepare the in-memory cache:
        let cache = InMemoryCache::builder()
            .resource_types(ResourceType::GUILD | ResourceType::CHANNEL | ResourceType::MESSAGE)
            .build();
        let cache = Arc::new(cache);
        self.cache = Some(cache.clone());

        // Gateway config:
        let config = Config::new(
            self.token.clone(),
            Intents::GUILDS | Intents::GUILD_MESSAGES | Intents::MESSAGE_CONTENT,
        );

        // Create recommended shards:
        let shards = gateway::create_recommended(&http_client, config, |_, b| b.build())
            .await
            .map_err(|e| Error::Platform(format!("create_recommended error: {e}")))?;

        for shard in shards {
            self.shard_senders.push(shard.sender());

            let tx_for_shard = tx.clone();
            let http_for_shard = http_client.clone();
            let bus_for_shard = self.event_bus.clone();
            let cache_for_shard = cache.clone();

            // Spawn the shard runner:
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
        self.connection_status = ConnectionStatus::Disconnected;

        // Gracefully close shards
        for sender in &self.shard_senders {
            let _ = sender.close(CloseFrame::NORMAL);
        }
        // Wait for them
        for task in &mut self.shard_tasks {
            let _ = task.await;
        }

        self.shard_senders.clear();
        self.shard_tasks.clear();

        // Optionally set rx back to None:
        {
            let mut guard = self.rx.lock().await;
            *guard = None;
        }

        Ok(())
    }

    async fn send_message(&self, channel: &str, message: &str) -> Result<(), Error> {
        let channel_id_u64: u64 = channel.parse().map_err(|_| {
            Error::Platform(format!("Invalid channel ID: {channel}"))
        })?;
        let channel_id = Id::<ChannelMarker>::new(channel_id_u64);

        if let Some(http) = &self.http {
            http.create_message(channel_id)
                .content(message)
                .await
                .map_err(|e| Error::Platform(format!("Error sending Discord message: {e:?}")))?;
        }

        Ok(())
    }

    async fn get_connection_status(&self) -> Result<ConnectionStatus, Error> {
        Ok(self.connection_status.clone())
    }
}