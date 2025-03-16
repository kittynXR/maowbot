// File: maowbot-core/src/platforms/discord/runtime.rs

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
use twilight_model::channel::ChannelType;
use twilight_model::gateway::payload::incoming::{
    MessageCreate, Ready as ReadyPayload,
};
use twilight_model::id::Id;

use crate::Error;
use crate::eventbus::EventBus;
use maowbot_common::traits::platform_traits::{ConnectionStatus, PlatformAuth, PlatformIntegration};

#[derive(Debug, Clone)]
pub struct DiscordMessageEvent {
    pub channel: String,
    pub user_id: String,
    pub username: String,
    pub text: String,
    pub user_roles: Vec<String>,
}

pub async fn shard_runner(
    mut shard: Shard,
    tx: UnboundedSender<DiscordMessageEvent>,
    http: Arc<HttpClient>,
    event_bus: Option<Arc<EventBus>>,
    cache: Arc<InMemoryCache>,
    // REMOVED: We no longer call upsert_* from events, so we remove the repo references:
    // pub discord_repo: Option<Arc<dyn DiscordRepository + Send + Sync>>,
    // pub account_name: String,
) {
    let shard_id = shard.id().number();
    info!("(ShardRunner) Shard {} started. Listening for events.", shard_id);

    while let Some(event_res) = shard.next_event(EventTypeFlags::all()).await {
        match event_res {
            Ok(event) => {
                // The cache tracks guilds/channels for ephemeral usage, but we won't commit them to DB here.
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
                            debug!("(ShardRunner) ignoring bot message from {}", msg.author.name);
                            continue;
                        }

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

                        let _ = tx.send(DiscordMessageEvent {
                            channel: channel_name,
                            user_id,
                            username,
                            text,
                            user_roles,
                        });

                        if let Some(bus) = &event_bus {
                            trace!("(EventBus) Not implemented for Discord chat yet");
                        }
                    }

                    // REMOVED: The upsert logic for guilds and channels to avoid “double sync”:
                    //
                    // Event::GuildCreate(guild_create_event) => { ... upsert_guild(...) ... }
                    // Event::ChannelCreate(ch_create_event)  => { ... upsert_channel(...) ... }
                    //
                    // We do not do DB writes here now. We rely on the manual "discord sync" to populate.

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

pub struct DiscordPlatform {
    token: String,
    connection_status: ConnectionStatus,

    rx: Option<UnboundedReceiver<DiscordMessageEvent>>,
    shard_tasks: Vec<JoinHandle<()>>,
    shard_senders: Vec<MessageSender>,

    pub(crate) http: Option<Arc<HttpClient>>,
    pub event_bus: Option<Arc<EventBus>>,
    pub cache: Option<Arc<InMemoryCache>>,

    // REMOVED: We can still store the DiscordRepository, but we no longer call it from shard_runner.
    // pub discord_repo: Option<Arc<dyn DiscordRepository + Send + Sync>>,
    // pub account_name: Option<String>,
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
            event_bus: None,
            cache: None,
        }
    }

    pub fn set_event_bus(&mut self, bus: Arc<EventBus>) {
        self.event_bus = Some(bus);
    }

    // Optionally keep a setter for the repository if you wish:
    /*
    pub fn set_discord_repository(
        &mut self,
        repo: Arc<dyn DiscordRepository + Send + Sync>,
        account_name: String
    ) {
        self.discord_repo = Some(repo);
        self.account_name = Some(account_name);
    }
    */

    pub async fn next_message_event(&mut self) -> Option<DiscordMessageEvent> {
        if let Some(rx) = &mut self.rx {
            rx.recv().await
        } else {
            None
        }
    }

    /// This manual sync can still be used externally from TUI or from any
    /// “bot_api.sync_discord_guilds_and_channels()” call.
    /// It fetches guilds + channels from the Twilight HTTP client and calls your repository.
    pub async fn sync_guilds_and_channels(
        &self,
        account_name: &str,
        discord_repo: &dyn maowbot_common::traits::repository_traits::DiscordRepository,
    ) -> Result<(), Error> {
        let http = match &self.http {
            Some(client) => client.clone(),
            None => {
                return Err(Error::Platform(
                    "No HTTP client available for DiscordPlatform".into()
                ));
            }
        };

        info!("sync_guilds_and_channels => fetching guilds for account='{account_name}'...");
        // 1) Get current user’s guilds
        let guilds_response = http.current_user_guilds().await.map_err(|e| {
            Error::Platform(format!("Discord HTTP error while listing guilds: {e}"))
        })?;

        let guilds_list = guilds_response.models().await.map_err(|e| {
            Error::Platform(format!("Discord parse error while listing guilds: {e}"))
        })?;

        info!(
            "Found {} guild(s) from the Discord API for account='{}'.",
            guilds_list.len(),
            account_name
        );

        // 2) For each guild, upsert them in DB, then fetch channels
        for g in guilds_list {
            let guild_id_str = g.id.to_string();
            let guild_name_str = g.name.clone();

            discord_repo.upsert_guild(account_name, &guild_id_str, &guild_name_str).await?;

            // Then fetch channels for that guild
            let channels_resp = http.guild_channels(g.id).await.map_err(|e| {
                Error::Platform(format!("Error fetching channels for guild {} => {e}", g.id))
            })?;
            let channels_list = channels_resp.models().await.map_err(|e| {
                Error::Platform(format!("Parse error for channels in guild {} => {e}", g.id))
            })?;

            for ch in channels_list {
                let channel_id_str = ch.id.to_string();
                let channel_name_str = ch
                    .name
                    .clone()
                    .unwrap_or_else(|| channel_id_str.clone());
                discord_repo.upsert_channel(
                    account_name,
                    &guild_id_str,
                    &channel_id_str,
                    &channel_name_str
                ).await?;
            }
        }

        Ok(())
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

        let shards = gateway::create_recommended(&http_client, config, |_, b| b.build())
            .await
            .map_err(|e| Error::Platform(format!("Error creating recommended shards: {e:?}")))?;

        let shard_count = shards.len();
        info!("(DiscordPlatform) create_recommended => {} shard(s).", shard_count);

        for shard in shards {
            self.shard_senders.push(shard.sender());

            let tx_for_shard = tx.clone();
            let http_for_shard = http_client.clone();
            let bus_for_shard = self.event_bus.clone();
            let cache_for_shard = cache.clone();

            // REMOVED: the local Arc<DiscordRepository>, account_name because
            // we do not do DB writes from event-handlers anymore.
            /*
            let repo_for_shard = self.discord_repo.clone();
            let acct_name = self.account_name.clone().unwrap_or_else(|| "UnknownAccount".into());
            */

            let handle = tokio::spawn(async move {
                shard_runner(
                    shard,
                    tx_for_shard,
                    http_for_shard,
                    bus_for_shard,
                    cache_for_shard,
                    // repo_for_shard,
                    // acct_name,
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
            http.create_message(channel_id)
                .content(message)
                .await
                .map_err(|err| Error::Platform(format!("Error sending Discord message: {err:?}")))?;
        } else {
            warn!("(DiscordPlatform) send_message => no HTTP client available?");
        }
        Ok(())
    }

    async fn get_connection_status(&self) -> Result<ConnectionStatus, Error> {
        Ok(self.connection_status.clone())
    }
}
