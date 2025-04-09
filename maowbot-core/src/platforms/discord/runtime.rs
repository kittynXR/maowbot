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
use twilight_http::client::ClientBuilder;
use twilight_http::Client as HttpClient;
use twilight_model::{
    channel::ChannelType,
    gateway::payload::incoming::{InteractionCreate, MessageCreate, Ready as ReadyPayload, PresenceUpdate},
    gateway::presence::ActivityType,
    id::marker::{ApplicationMarker, ChannelMarker, GuildMarker, RoleMarker, UserMarker},
};
use twilight_model::util::Timestamp;
use twilight_util::builder::embed::ImageSource;
use maowbot_common::error::Error;
use maowbot_common::traits::platform_traits::{ConnectionStatus, PlatformAuth, PlatformIntegration};

use crate::eventbus::EventBus;
use crate::services::discord::slashcommands;

/// Represents inbound chat message data (not slash commands).
#[derive(Debug, Clone)]
pub struct DiscordMessageEvent {
    pub channel: String,
    pub user_id: String,
    pub username: String,
    pub text: String,
    pub user_roles: Vec<String>,
    pub guild_id: Option<String>,
}

/// The shard runner reads gateway events and updates the cache.
async fn shard_runner(
    mut shard: Shard,
    tx: UnboundedSender<DiscordMessageEvent>,
    http: Arc<HttpClient>,
    event_bus: Option<Arc<EventBus>>,
    cache: Arc<InMemoryCache>,
    application_id: Option<twilight_model::id::Id<ApplicationMarker>>,
    discord_repo: Option<Arc<dyn maowbot_common::traits::repository_traits::DiscordRepository + Send + Sync>>,
) {
    let shard_id = shard.id().number();
    info!("(ShardRunner) Shard {shard_id} started. Listening for events.");

    while let Some(item) = shard.next_event(EventTypeFlags::all()).await {
        match item {
            Ok(event) => {
                // Update the in-memory cache with each event
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
                        // Ignore bot messages:
                        if msg.author.bot {
                            debug!("Ignoring bot message from {}", msg.author.name);
                            continue;
                        }

                        // Using .await? style in 0.16:
                        let channel_resp = match http.channel(msg.channel_id).await {
                            Ok(resp) => resp, // a Response<Channel>
                            Err(e) => {
                                error!("Error fetching channel => {e:?}");
                                continue;
                            }
                        };

                        // Attempt to deserialize the channel
                        let ch_obj = match channel_resp.model().await {
                            Ok(ch) => ch,
                            Err(_) => {
                                // fallback if we fail to parse
                                twilight_model::channel::Channel {
                                    id: msg.channel_id,
                                    invitable: None,
                                    kind: ChannelType::Unknown(0),
                                    name: None,
                                    last_message_id: None,
                                    last_pin_timestamp: None,
                                    parent_id: None,
                                    permission_overwrites: None,
                                    position: None,
                                    bitrate: None,
                                    default_auto_archive_duration: None,
                                    default_forum_layout: None,
                                    default_reaction_emoji: None,
                                    default_sort_order: None,
                                    default_thread_rate_limit_per_user: None,
                                    user_limit: None,
                                    rate_limit_per_user: None,
                                    recipients: None,
                                    rtc_region: None,
                                    nsfw: None,
                                    topic: None,
                                    guild_id: None,
                                    icon: None,
                                    owner_id: None,
                                    application_id,
                                    applied_tags: None,
                                    managed: None,
                                    member: None,
                                    member_count: None,
                                    message_count: None,
                                    available_tags: None,
                                    flags: None,
                                    newly_created: None,
                                    thread_metadata: None,
                                    video_quality_mode: None,
                                }
                            }
                        };

                        let channel_name = match ch_obj.kind {
                            ChannelType::GuildText
                            | ChannelType::GuildVoice
                            | ChannelType::GuildForum
                            | ChannelType::GuildStageVoice => {
                                ch_obj.name.unwrap_or_else(|| msg.channel_id.to_string())
                            }
                            ChannelType::Private => format!("(DM {})", msg.channel_id),
                            ChannelType::Group => "(Group DM)".to_string(),
                            _ => msg.channel_id.to_string(),
                        };

                        let user_roles: Vec<String> = msg
                            .member
                            .as_ref()
                            .map(|m| m.roles.iter().map(|r| r.to_string()).collect())
                            .unwrap_or_default();

                        let guild_id = ch_obj.guild_id.map(|id| id.to_string());
                        
                        let _ = tx.send(DiscordMessageEvent {
                            channel: channel_name,
                            user_id: msg.author.id.to_string(),
                            username: msg.author.name.clone(),
                            text: msg.content.clone(),
                            user_roles,
                            guild_id,
                        });
                    }
                    Event::InteractionCreate(inter_create) => {
                        if let Some(app_id) = application_id {
                            // Dispatch slash command
                            if let Err(e) = slashcommands::handle_interaction_create(
                                http.clone(),
                                app_id,
                                inter_create,
                            )
                                .await
                            {
                                error!("Slash command error => {e:?}");
                            }
                        }
                    }
                    Event::PresenceUpdate(presence_update) => {
                        // Handle presence update for live role
                        if let Some(repo) = &discord_repo {
                            let guild_id = presence_update.guild_id.to_string();
                            let user_id = presence_update.user.id().to_string();
                            
                            // Try to get the live role for this guild
                            match repo.get_live_role(&guild_id).await {
                                Ok(Some(live_role)) => {
                                    // First check if there was a change in streaming status
                                    let is_streaming = presence_update.activities.iter().any(|activity| {
                                        activity.kind == ActivityType::Streaming && 
                                        activity.url.as_ref().map_or(false, |url| url.contains("twitch.tv"))
                                    });
                                    
                                    // Convert string IDs to u64 and create Twilight IDs
                                    // We'll need these for both adding and checking roles
                                    let guild_id_u64 = guild_id.parse::<u64>().unwrap_or_else(|_| {
                                        warn!("Invalid guild ID format: {}", guild_id);
                                        0
                                    });
                                    let user_id_u64 = user_id.parse::<u64>().unwrap_or_else(|_| {
                                        warn!("Invalid user ID format: {}", user_id);
                                        0
                                    });
                                    let role_id_u64 = live_role.role_id.parse::<u64>().unwrap_or_else(|_| {
                                        warn!("Invalid role ID format: {}", live_role.role_id);
                                        0
                                    });
                                    
                                    // Skip if any ID parsing failed
                                    if guild_id_u64 == 0 || user_id_u64 == 0 || role_id_u64 == 0 {
                                        continue;
                                    }
                                    
                                    let guild_id = twilight_model::id::Id::<GuildMarker>::new(guild_id_u64);
                                    let user_id = twilight_model::id::Id::<UserMarker>::new(user_id_u64);
                                    let role_id = twilight_model::id::Id::<RoleMarker>::new(role_id_u64);
                                    
                                    // Check if the member has the role using the cache
                                    let has_role = if let Some(member) = cache.member(guild_id, user_id) {
                                        member.roles().iter().any(|&r| r == role_id)
                                    } else {
                                        false
                                    };
                                    
                                    // Only perform action if there's a status change
                                    if is_streaming && !has_role {
                                        // User is streaming but doesn't have the role - add it
                                        debug!("User {} started streaming on Twitch, adding live role {}", 
                                            user_id, live_role.role_id);
                                        
                                        if let Err(e) = http.add_guild_member_role(guild_id, user_id, role_id).await {
                                            warn!("Failed to add live role to user: {}", e);
                                        }
                                    } else if !is_streaming && has_role {
                                        // User has stopped streaming and has the role - remove it
                                        debug!("User {} stopped streaming on Twitch, removing live role {}", 
                                            user_id, live_role.role_id);
                                        
                                        if let Err(e) = http.remove_guild_member_role(guild_id, user_id, role_id).await {
                                            warn!("Failed to remove live role from user: {}", e);
                                        }
                                    } else {
                                        // No change in streaming status or role state matches status
                                        trace!("No change in streaming status for user {}", user_id);
                                    }
                                }
                                Ok(None) => {
                                    // No live role configured for this guild
                                    trace!("No live role configured for guild {}", guild_id);
                                }
                                Err(e) => {
                                    warn!("Error checking for live role: {}", e);
                                }
                            }
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

/// Primary struct for your Discord integration.
pub struct DiscordPlatform {
    pub token: String,
    pub connection_status: ConnectionStatus,
    /// For normal chat messages
    pub rx: Mutex<Option<UnboundedReceiver<DiscordMessageEvent>>>,
    /// Each shard spawns a task
    pub shard_tasks: Vec<JoinHandle<()>>,
    pub shard_senders: Vec<MessageSender>,
    /// The Twilight HTTP client
    pub http: Option<Arc<HttpClient>>,
    /// The in-memory cache as an Arc
    pub cache: Option<Arc<InMemoryCache>>,
    /// Optional event bus for global broadcast
    pub event_bus: Option<Arc<EventBus>>,
    /// If set, the ID for slash commands
    pub application_id: Option<twilight_model::id::Id<ApplicationMarker>>,
    /// Reference to the Discord repository for live role functionality
    pub discord_repo: Option<Arc<dyn maowbot_common::traits::repository_traits::DiscordRepository + Send + Sync>>,
}

impl DiscordPlatform {
    pub fn new(token: String) -> Self {
        Self {
            token,
            connection_status: ConnectionStatus::Disconnected,
            rx: Mutex::new(None),
            shard_tasks: Vec::new(),
            shard_senders: Vec::new(),
            http: None,
            cache: None,
            event_bus: None,
            application_id: None,
            discord_repo: None,
        }
    }
    
    pub fn set_discord_repo(&mut self, repo: Arc<dyn maowbot_common::traits::repository_traits::DiscordRepository + Send + Sync>) {
        self.discord_repo = Some(repo);
    }

    pub fn set_event_bus(&mut self, bus: Arc<EventBus>) {
        self.event_bus = Some(bus);
    }

    pub fn set_application_id_from_refresh_token(&mut self, refresh_token: &str) -> Result<(), Error> {
        let app_id = refresh_token.parse::<u64>()
            .map_err(|e| Error::Platform(format!("Failed to parse application id from refresh token: {e}")))?;
        self.application_id = Some(twilight_model::id::Id::new(app_id));
        Ok(())
    }

    /// Wait for the next inbound chat message.
    pub async fn next_message_event(&self) -> Option<DiscordMessageEvent> {
        let mut guard = self.rx.lock().await;
        guard.as_mut()?.recv().await
    }
    /// Sends a Discord embed
    pub async fn send_embed(
        &self,
        channel_id: twilight_model::id::Id<ChannelMarker>,
        embed: &maowbot_common::models::discord::DiscordEmbed,
        content: Option<&str>,
    ) -> Result<(), Error> {
        if let Some(http) = &self.http {
            // Begin creating the message
            let mut message_builder = http.create_message(channel_id);

            // Add content if provided
            if let Some(content_text) = content {
                message_builder = message_builder.content(content_text);
            }

            // Create a Twilight embed builder with the correct structure
            use twilight_util::builder::embed::{
                EmbedBuilder, EmbedAuthorBuilder, EmbedFieldBuilder, EmbedFooterBuilder,
            };

            let mut embed_builder = EmbedBuilder::new();

            // Add basic fields
            if let Some(title) = &embed.title {
                embed_builder = embed_builder.title(title);
            }

            if let Some(description) = &embed.description {
                embed_builder = embed_builder.description(description);
            }

            if let Some(url) = &embed.url {
                embed_builder = embed_builder.url(url);
            }

            // Handle timestamp - convert from DateTime<Utc> to Timestamp
            if let Some(timestamp) = &embed.timestamp {
                // Convert DateTime<Utc> to Timestamp using parse
                match Timestamp::parse(&timestamp.to_rfc3339()) {
                    Ok(ts) => {
                        embed_builder = embed_builder.timestamp(ts);
                    },
                    Err(e) => {
                        return Err(Error::Platform(format!("Failed to parse timestamp: {}", e)));
                    }
                }
            }

            if let Some(color) = &embed.color {
                embed_builder = embed_builder.color(color.0);
            }

            // Add author if present
            if let Some(author) = &embed.author {
                let mut author_builder = EmbedAuthorBuilder::new(author.name.clone());

                if let Some(author_url) = &author.url {
                    author_builder = author_builder.url(author_url);
                }

                if let Some(icon_url) = &author.icon_url {
                    // Convert URL string to ImageSource safely without using ?
                    match ImageSource::url(icon_url) {
                        Ok(img) => {
                            author_builder = author_builder.icon_url(img);
                        },
                        Err(e) => {
                            return Err(Error::Platform(format!("Invalid author icon URL: {}", e)));
                        }
                    }
                }

                embed_builder = embed_builder.author(author_builder.build());
            }

            // Add footer if present
            if let Some(footer) = &embed.footer {
                let mut footer_builder = EmbedFooterBuilder::new(footer.text.clone());

                if let Some(icon_url) = &footer.icon_url {
                    // Convert URL string to ImageSource safely
                    match ImageSource::url(icon_url) {
                        Ok(img) => {
                            footer_builder = footer_builder.icon_url(img);
                        },
                        Err(e) => {
                            return Err(Error::Platform(format!("Invalid footer icon URL: {}", e)));
                        }
                    }
                }

                embed_builder = embed_builder.footer(footer_builder.build());
            }

            // Add image if present
            if let Some(image) = &embed.image {
                // Convert the URL string to an ImageSource without using ?
                match ImageSource::url(&image.url) {
                    Ok(img) => {
                        embed_builder = embed_builder.image(img);
                    },
                    Err(e) => {
                        return Err(Error::Platform(format!("Invalid image URL: {}", e)));
                    }
                }
            }

            // Add thumbnail if present
            if let Some(thumbnail) = &embed.thumbnail {
                // Convert the URL string to an ImageSource without using ?
                match ImageSource::url(&thumbnail.url) {
                    Ok(img) => {
                        embed_builder = embed_builder.thumbnail(img);
                    },
                    Err(e) => {
                        return Err(Error::Platform(format!("Invalid thumbnail URL: {}", e)));
                    }
                }
            }

            // Add fields
            for field in &embed.fields {
                let mut field_builder = EmbedFieldBuilder::new(
                    field.name.clone(),
                    field.value.clone()
                );

                // Call .inline() only if the field should be inline
                if field.inline {
                    field_builder = field_builder.inline();
                }

                embed_builder = embed_builder.field(field_builder.build());
            }

            // Build the embed
            let built_embed = embed_builder.build();

            // Create a longer-lived array that won't be dropped
            let embeds = [built_embed];
            message_builder = message_builder.embeds(&embeds);

            // Send the message
            message_builder
                .await
                .map_err(|e| Error::Platform(format!("Failed to send Discord embed: {}", e)))?;
        }

        Ok(())
    }
    pub async fn send_channel_embed(
        &self,
        channel_id_str: &str,
        embed: &maowbot_common::models::discord::DiscordEmbed,
        content: Option<&str>
    ) -> Result<(), Error> {
        // Channel must be a numeric ID for Discord API
        if !channel_id_str.chars().all(|c| c.is_ascii_digit()) {
            return Err(Error::Platform(format!("Channel must be an ID, but got a name: {}", channel_id_str)));
        }
        
        let channel_id_u64: u64 = channel_id_str.parse().map_err(|_| {
            Error::Platform(format!("Invalid channel ID: {}", channel_id_str))
        })?;
        let channel_id = twilight_model::id::Id::<ChannelMarker>::new(channel_id_u64);

        self.send_embed(channel_id, embed, content).await
    }
}

#[async_trait]
impl PlatformAuth for DiscordPlatform {
    async fn authenticate(&mut self) -> Result<(), Error> {
        if self.token.is_empty() {
            return Err(Error::Auth("Discord token is empty.".into()));
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

#[async_trait]
impl PlatformIntegration for DiscordPlatform {
    async fn connect(&mut self) -> Result<(), Error> {
        if matches!(self.connection_status, ConnectionStatus::Connected) {
            info!("(DiscordPlatform) Already connected => skipping");
            return Ok(());
        }

        // For normal chat messages
        let (tx, rx) = unbounded_channel::<DiscordMessageEvent>();
        {
            let mut guard = self.rx.lock().await;
            *guard = Some(rx);
        }

        // Build the Twilight HTTP client
        let http_client = Arc::new(
            ClientBuilder::new()
                .token(self.token.clone())
                .timeout(Duration::from_secs(30))
                .build(),
        );
        self.http = Some(http_client.clone());

        // Build the in-memory cache as an Arc, including ROLE resource type
        let cache = InMemoryCache::builder()
            .resource_types(ResourceType::GUILD | ResourceType::CHANNEL | ResourceType::MESSAGE | ResourceType::ROLE)
            .build();
        let arc_cache = Arc::new(cache);
        self.cache = Some(arc_cache.clone());

        // If we have an application_id, register slash commands
        if let Some(app_id) = self.application_id {
            if let Err(e) = slashcommands::register_global_slash_commands(&http_client, app_id).await {
                error!("Failed to register slash commands => {e:?}");
            }
        }

        // Create recommended shards
        let config = Config::new(
            self.token.clone(),
            Intents::GUILDS | Intents::GUILD_MESSAGES | Intents::MESSAGE_CONTENT | Intents::GUILD_PRESENCES,
        );

        let shards = gateway::create_recommended(&http_client, config, |_, b| b.build())
            .await
            .map_err(|e| Error::Platform(format!("create_recommended error: {e}")))?;

        // Spawn each shard
        for shard in shards {
            self.shard_senders.push(shard.sender());

            let tx_for_shard = tx.clone();
            let http_for_shard = http_client.clone();
            let bus_for_shard = self.event_bus.clone();
            let cache_for_shard = arc_cache.clone();
            let app_id = self.application_id;
            let discord_repo_for_shard = self.discord_repo.clone();

            let handle = tokio::spawn(async move {
                shard_runner(
                    shard,
                    tx_for_shard,
                    http_for_shard,
                    bus_for_shard,
                    cache_for_shard,
                    app_id,
                    discord_repo_for_shard,
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

        // Gracefully close
        for sender in &self.shard_senders {
            let _ = sender.close(CloseFrame::NORMAL);
        }
        for task in &mut self.shard_tasks {
            let _ = task.await;
        }
        self.shard_senders.clear();
        self.shard_tasks.clear();

        // Clear inbound channel
        {
            let mut guard = self.rx.lock().await;
            *guard = None;
        }

        Ok(())
    }

    /// For 0.16, `.content(...)` is not a `Result`. No `?` needed.
    async fn send_message(&self, channel: &str, message: &str) -> Result<(), Error> {
        // Channel must be a numeric ID for Discord API
        if !channel.chars().all(|c| c.is_ascii_digit()) {
            return Err(Error::Platform(format!("Channel must be an ID, but got a name: {}", channel)));
        }
        
        let channel_id_u64: u64 = channel.parse().map_err(|_| {
            Error::Platform(format!("Invalid channel ID: {}", channel))
        })?;
        let channel_id = twilight_model::id::Id::<ChannelMarker>::new(channel_id_u64);

        if let Some(http) = &self.http {
            http.create_message(channel_id)
                .content(message)
                // `.content(...)` is not a Result in Twilight 0.16,
                // so no `.map_err(...)` or `?`.
                .await
                .map_err(|e| Error::Platform(format!("Failed to send Discord message: {e}")))?;
        }

        Ok(())
    }

    async fn get_connection_status(&self) -> Result<ConnectionStatus, Error> {
        Ok(self.connection_status.clone())
    }
}

impl DiscordPlatform {
    /// Add a role to a Discord user
    pub async fn add_role_to_user(
        &self,
        guild_id: &str,
        user_id: &str,
        role_id: &str
    ) -> Result<(), Error> {
        if let Some(http) = &self.http {
            let guild_id_u64: u64 = guild_id.parse().map_err(|_| {
                Error::Platform(format!("Invalid guild ID: {}", guild_id))
            })?;
            
            let user_id_u64: u64 = user_id.parse().map_err(|_| {
                Error::Platform(format!("Invalid user ID: {}", user_id))
            })?;
            
            let role_id_u64: u64 = role_id.parse().map_err(|_| {
                Error::Platform(format!("Invalid role ID: {}", role_id))
            })?;
            
            let guild_id = twilight_model::id::Id::<GuildMarker>::new(guild_id_u64);
            let user_id = twilight_model::id::Id::<UserMarker>::new(user_id_u64);
            let role_id = twilight_model::id::Id::<RoleMarker>::new(role_id_u64);
            
            http.add_guild_member_role(guild_id, user_id, role_id)
                .await
                .map_err(|e| Error::Platform(format!("Failed to add role to user: {}", e)))?;
        }
        
        Ok(())
    }
    
    /// Remove a role from a Discord user
    pub async fn remove_role_from_user(
        &self,
        guild_id: &str,
        user_id: &str,
        role_id: &str
    ) -> Result<(), Error> {
        if let Some(http) = &self.http {
            let guild_id_u64: u64 = guild_id.parse().map_err(|_| {
                Error::Platform(format!("Invalid guild ID: {}", guild_id))
            })?;
            
            let user_id_u64: u64 = user_id.parse().map_err(|_| {
                Error::Platform(format!("Invalid user ID: {}", user_id))
            })?;
            
            let role_id_u64: u64 = role_id.parse().map_err(|_| {
                Error::Platform(format!("Invalid role ID: {}", role_id))
            })?;
            
            let guild_id = twilight_model::id::Id::<GuildMarker>::new(guild_id_u64);
            let user_id = twilight_model::id::Id::<UserMarker>::new(user_id_u64);
            let role_id = twilight_model::id::Id::<RoleMarker>::new(role_id_u64);
            
            http.remove_guild_member_role(guild_id, user_id, role_id)
                .await
                .map_err(|e| Error::Platform(format!("Failed to remove role from user: {}", e)))?;
        }
        
        Ok(())
    }
}
