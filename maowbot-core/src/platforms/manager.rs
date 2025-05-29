use std::collections::HashMap;
use std::sync::Arc;
use tokio::task::JoinHandle;
use tracing::{info, error, warn, debug};
use tokio::sync::Mutex as AsyncMutex;
use std::sync::Mutex;
use twilight_cache_inmemory::InMemoryCache;
use maowbot_common::models::discord::DiscordEmbed;
use maowbot_common::models::platform::{Platform, PlatformCredential};
use maowbot_common::traits::platform_traits::{ChatPlatform, ConnectionStatus, PlatformIntegration};
use maowbot_common::traits::repository_traits::CredentialsRepository;
use crate::eventbus::EventBus;
use crate::services::message_service::MessageService;
use crate::services::user_service::UserService;
use crate::Error;

use crate::platforms::discord::runtime::DiscordPlatform;
use crate::platforms::twitch::client::TwitchHelixClient;
use crate::platforms::twitch::runtime::TwitchPlatform;
use crate::platforms::vrchat_pipeline::runtime::VRChatPlatform;
use crate::platforms::twitch_irc::runtime::TwitchIrcPlatform;
use crate::platforms::twitch_eventsub::runtime::TwitchEventSubPlatform;
use crate::repositories::postgres::discord::PostgresDiscordRepository;

pub struct PlatformRuntimeHandle {
    pub join_handle: JoinHandle<()>,
    pub platform: String,
    pub user_id: String,

    pub twitch_irc_instance: Option<Arc<AsyncMutex<TwitchIrcPlatform>>>,
    pub vrchat_instance: Option<Arc<AsyncMutex<VRChatPlatform>>>,
    pub discord_instance: Option<Arc<DiscordPlatform>>,
}

/// Manages starting/stopping platform runtimes, holding references to them, etc.
pub struct PlatformManager {
    message_service: Mutex<Option<Arc<MessageService>>>,
    user_svc: Arc<UserService>,
    event_bus: Arc<EventBus>,
    pub(crate) credentials_repo: Arc<dyn CredentialsRepository + Send + Sync>,

    pub active_runtimes: AsyncMutex<HashMap<(String, String), PlatformRuntimeHandle>>,
    pub discord_caches: AsyncMutex<HashMap<(String, String), Arc<InMemoryCache>>>,
    pub discord_repo: Arc<PostgresDiscordRepository>,
    
    // Reference to the plugin manager - will be set later
    plugin_manager: Mutex<Option<Arc<crate::plugins::manager::PluginManager>>>,
}

impl PlatformManager {
    pub fn new(
        user_svc: Arc<UserService>,
        event_bus: Arc<EventBus>,
        credentials_repo: Arc<dyn CredentialsRepository + Send + Sync>,
        discord_repo: Arc<PostgresDiscordRepository>,
    ) -> Self {
        Self {
            message_service: Mutex::new(None),
            user_svc,
            event_bus,
            credentials_repo,
            active_runtimes: AsyncMutex::new(HashMap::new()),
            discord_caches: AsyncMutex::new(HashMap::new()),
            discord_repo,
            plugin_manager: Mutex::new(None),
        }
    }
    
    /// Set the plugin manager reference
    pub fn set_plugin_manager(&self, plugin_manager: Arc<crate::plugins::manager::PluginManager>) {
        let mut pm = self.plugin_manager.lock().unwrap();
        *pm = Some(plugin_manager);
    }
    
    /// Get the plugin manager reference
    pub fn plugin_manager(&self) -> Option<Arc<crate::plugins::manager::PluginManager>> {
        let pm = self.plugin_manager.lock().unwrap();
        pm.clone()
    }
    
    /// Get access to the AI API through the plugin manager
    pub fn get_ai_api(&self) -> Option<Arc<dyn maowbot_common::traits::api::AiApi + Send + Sync>> {
        // First attempt: try to get from plugin_manager if it exists
        if let Some(pm) = self.plugin_manager() {
            if let Some(ai_impl) = &pm.ai_api_impl {
                info!("Found AI API implementation in plugin_manager");
                return Some(Arc::new(ai_impl.clone()));
            }
        }
        
        // If we reach here, we couldn't get the AI API from plugin_manager
        info!("Could not find AI API implementation in plugin_manager, falling back to stub");
        None
    }

    pub fn set_message_service(&self, svc: Arc<MessageService>) {
        let mut guard = self.message_service.lock().unwrap();
        *guard = Some(svc);
    }

    fn get_message_service(&self) -> Result<Arc<MessageService>, Error> {
        let guard = self.message_service.lock().unwrap();
        if let Some(ms) = &*guard {
            Ok(ms.clone())
        } else {
            Err(Error::Platform("No message_service set in PlatformManager.".into()))
        }
    }

    /// Starts the bot’s runtime for a given platform + account, if not already running.
    pub async fn start_platform_runtime(
        &self,
        platform_str: &str,
        account_name: &str,
    ) -> Result<(), Error> {
        let user = self.user_svc
            .find_user_by_global_username(account_name)
            .await?;

        let platform = platform_str.parse::<Platform>()
            .map_err(|_| Error::Platform(format!("Unknown platform '{platform_str}'")))?;

        let creds_opt = self.credentials_repo
            .get_credentials(&platform, user.user_id)
            .await?;
        let creds = match creds_opt {
            Some(c) => c,
            None => {
                return Err(Error::Auth(format!(
                    "No credentials for user='{account_name}' and platform='{platform_str}'",
                )));
            }
        };

        let key = (platform_str.to_string(), user.user_id.to_string());
        {
            let guard = self.active_runtimes.lock().await;
            if guard.contains_key(&key) {
                info!("Runtime already running for platform='{platform_str}' user_id='{}'.", user.user_id);
                return Ok(());
            }
        }

        let handle = match platform {
            Platform::Discord => self.spawn_discord(creds).await?,
            Platform::Twitch => self.spawn_twitch_helix(creds).await?,
            Platform::VRChat => self.spawn_vrchat(creds).await?,
            Platform::TwitchIRC => self.spawn_twitch_irc(creds).await?,
            Platform::TwitchEventSub => self.spawn_twitch_eventsub(creds).await?,
        };

        {
            let mut guard = self.active_runtimes.lock().await;
            guard.insert(key, handle);
        }

        Ok(())
    }

    pub async fn stop_platform_runtime(
        &self,
        platform_str: &str,
        account_name: &str,
    ) -> Result<(), Error> {
        let user = self.user_svc
            .find_user_by_global_username(account_name)
            .await?;
        let key = (platform_str.to_string(), user.user_id.to_string());

        let handle_opt = {
            let mut guard = self.active_runtimes.lock().await;
            guard.remove(&key)
        };
        if let Some(rh) = handle_opt {
            rh.join_handle.abort();
            info!("Stopped runtime for platform='{platform_str}', user_id={}", user.user_id);
        } else {
            warn!("No active runtime for platform='{platform_str}', account='{account_name}'");
        }
        Ok(())
    }

    pub async fn get_discord_platform(
        &self,
        account_name: &str
    ) -> Result<Arc<DiscordPlatform>, Error> {
        // This is just an alias for get_discord_instance for better API clarity
        self.get_discord_instance(account_name).await
    }
    
    pub async fn get_discord_instance(
        &self,
        account_name: &str
    ) -> Result<Arc<DiscordPlatform>, Error> {
        let user = self.user_svc.find_user_by_global_username(account_name).await?;
        let key = ("discord".to_string(), user.user_id.to_string());
        let guard = self.active_runtimes.lock().await;
        if let Some(handle) = guard.get(&key) {
            if let Some(discord_arc) = &handle.discord_instance {
                Ok(Arc::clone(discord_arc))
            } else {
                Err(Error::Platform(format!(
                    "No DiscordPlatform instance found for account='{account_name}'"
                )))
            }
        } else {
            Err(Error::Platform(format!(
                "No active Discord runtime for account='{account_name}'"
            )))
        }
    }

    pub async fn get_twitch_client(&self) -> Option<TwitchHelixClient> {
        match self.credentials_repo.get_broadcaster_credential(&Platform::Twitch).await {
            Ok(Some(cred)) => {
                if let Some(additional_data) = cred.additional_data {
                    if let Some(client_id_val) = additional_data.get("client_id") {
                        if let Some(client_id_str) = client_id_val.as_str() {
                            return Some(TwitchHelixClient::new(&cred.primary_token, client_id_str));
                        }
                    }
                }
                None
            },
            _ => None,
        }
    }


    async fn spawn_discord(&self, credential: PlatformCredential) -> Result<PlatformRuntimeHandle, Error> {
        let msg_svc = self.get_message_service()?;

        let user_id_str = credential.user_id.to_string();
        let token = credential.primary_token.clone();

        // We create the DiscordPlatform:
        let mut discord = DiscordPlatform::new(token);

        if let Some(app_id_str) = &credential.refresh_token {
            discord.set_application_id_from_refresh_token(app_id_str)?;
        }

        discord.set_event_bus(self.event_bus.clone());
        discord.set_discord_repo(self.discord_repo.clone());
        discord.connect().await?;

        // We pull out its Arc<InMemoryCache> so we can store it in `discord_caches`:
        let cache = discord.cache.clone()
            .ok_or_else(|| Error::Platform("DiscordPlatform missing in-memory cache".into()))?;

        // For inbound messages, just spawn a loop reading next_message_event:
        let cloned_discord = Arc::new(discord);
        let join_handle = tokio::spawn({
            let cloned_discord2 = cloned_discord.clone();
            async move {
                loop {
                    match cloned_discord2.next_message_event().await {
                        Some(msg_event) => {
                            // Include guild_id in metadata if available
                            let metadata: Vec<String> = if let Some(guild_id) = &msg_event.guild_id {
                                vec![format!("guild_id:{}", guild_id)]
                            } else {
                                Vec::new()
                            };
                            
                            if let Err(e) = msg_svc
                                .process_incoming_message(
                                    "discord",
                                    &msg_event.channel,
                                    &msg_event.user_id,
                                    Some(&msg_event.username),
                                    &msg_event.user_roles,
                                    &msg_event.text,
                                    &metadata
                                )
                                .await
                            {
                                tracing::error!("Discord message error: {e}");
                            }
                        }
                        None => break,
                    }
                }
            }
        });

        // Insert the Arc<InMemoryCache> and the handle into our manager
        let key = ("discord".to_string(), user_id_str.clone());
        {
            let mut lock = self.discord_caches.lock().await;
            lock.insert(key.clone(), cache);
        }

        Ok(PlatformRuntimeHandle {
            join_handle,
            platform: "discord".into(),
            user_id: user_id_str,
            discord_instance: Some(cloned_discord),
            twitch_irc_instance: None,
            vrchat_instance: None,
        })
    }

    pub async fn get_discord_cache(
        &self,
        account_name: &str
    ) -> Result<Arc<InMemoryCache>, Error> {
        let user = self.user_svc
            .find_user_by_global_username(account_name)
            .await?;
        let key = ("discord".to_string(), user.user_id.to_string());

        let lock = self.discord_caches.lock().await;
        if let Some(cache) = lock.get(&key) {
            Ok(Arc::clone(cache))
        } else {
            Err(Error::Platform(format!(
                "No Discord in-memory cache found for account='{account_name}'"
            )))
        }
    }

    async fn spawn_twitch_helix(&self, credential: PlatformCredential) -> Result<PlatformRuntimeHandle, Error> {
        let message_svc = self.get_message_service()?;

        let user_id_str = credential.user_id.to_string();
        let user_id_str_for_handle = user_id_str.clone();
        let user_id_str_for_closure = user_id_str.clone();

        let join_handle = tokio::spawn(async move {
            let mut twitch = TwitchPlatform {
                credentials: Some(credential.clone()),
                connection_status: ConnectionStatus::Disconnected,
                client: None,
            };
            if let Err(err) = twitch.connect().await {
                error!("[TwitchHelix] connect error: {err:?}");
                return;
            }
            info!("[TwitchHelix] Connected for user_id={}", user_id_str_for_closure);

            while let Some(msg_event) = twitch.next_message_event().await {
                let channel = msg_event.channel;
                let display_name = msg_event.display_name.clone();
                let platform_user_id = msg_event.user_id;
                let text = msg_event.text;

                if let Err(e) = message_svc
                    .process_incoming_message(
                        "twitch",
                        &channel,
                        &platform_user_id,
                        Some(&display_name),
                        &[],
                        &text,
                        &[],
                    )
                    .await
                {
                    error!("[TwitchHelix] process_incoming_message => {e:?}");
                }
            }

            info!("[TwitchHelix] Task ended for user_id={}", user_id_str_for_closure);
        });

        Ok(PlatformRuntimeHandle {
            join_handle,
            platform: "twitch".into(),
            user_id: user_id_str_for_handle,
            twitch_irc_instance: None,
            vrchat_instance: None,
            discord_instance: None,
        })
    }

    async fn spawn_vrchat(&self, credential: PlatformCredential) -> Result<PlatformRuntimeHandle, Error> {
        let message_svc = self.get_message_service()?;

        let user_id_str = credential.user_id.to_string();
        let user_id_str_for_handle = user_id_str.clone();
        let user_id_str_for_closure = user_id_str.clone();

        let mut vrc = VRChatPlatform::new();
        vrc.credentials = Some(credential);

        let arc_vrc = Arc::new(AsyncMutex::new(vrc));
        let cloned_vrc = arc_vrc.clone();

        let join_handle = tokio::spawn(async move {
            if let Err(err) = cloned_vrc.lock().await.connect().await {
                error!("[VRChat] connect error: {err:?}");
                return;
            }
            info!("[VRChat] Connected for user_id={}", user_id_str_for_closure);

            while let Some(evt) = cloned_vrc.lock().await.next_message_event().await {
                let channel = "someVrcRoom";
                let platform_user_id = evt.user_id.clone();
                let display_name = evt.vrchat_display_name.clone();
                let text = evt.text;

                if let Err(e) = message_svc
                    .process_incoming_message(
                        "vrchat",
                        channel,
                        &platform_user_id,
                        Some(&display_name),
                        &[],
                        &text,
                        &[],
                    )
                    .await
                {
                    error!("[VRChat] process_incoming_message => {e:?}");
                }
            }

            info!("[VRChat] Task ended for user_id={}", user_id_str_for_closure);
        });

        Ok(PlatformRuntimeHandle {
            join_handle,
            platform: "vrchat".into(),
            user_id: user_id_str_for_handle,
            twitch_irc_instance: None,
            vrchat_instance: Some(arc_vrc),
            discord_instance: None,
        })
    }

    async fn spawn_twitch_irc(&self, credential: PlatformCredential) -> Result<PlatformRuntimeHandle, Error> {
        let message_svc = self.get_message_service()?;
        let user_id_str = credential.user_id.to_string();
        let user_id_str_for_handle = user_id_str.clone();
        let user_id_str_for_closure = user_id_str.clone();

        let mut irc = TwitchIrcPlatform::new();
        irc.set_credentials(credential.clone());
        irc.set_event_bus(self.event_bus.clone());

        // If this credential is a bot, we can choose whether to skip reading or not:
        if credential.is_bot == true {
            // For a bot account, we typically want to read commands too, so let's leave
            // enable_incoming = true if we do want them to handle them.
            // However, the example might set false. For now we keep it true.
            // irc.enable_incoming = false; // if we wanted it to ignore inbound, we’d do this
        }

        irc.connect().await?;
        info!("[TwitchIRC] connected for user_id={}", user_id_str_for_closure);

        // ---- NEW: let this TTV-IRC account join all other Twitch accounts’ channels ----
        self.join_all_twitch_channels(&irc, credential.user_id).await?;

        let rx_opt = irc.rx.take();
        let arc_irc = Arc::new(AsyncMutex::new(irc));

        let join_handle = tokio::spawn(async move {
            if let Some(mut msg_rx) = rx_opt {
                while let Some(evt) = msg_rx.recv().await {
                    let channel = evt.channel;
                    let platform_user_id = evt.twitch_user_id.clone();
                    let display_name = evt.display_name.clone();
                    let roles = evt.roles.clone();
                    let text = evt.text;

                    if let Err(e) = message_svc
                        .process_incoming_message(
                            "twitch-irc",
                            &channel,
                            &platform_user_id,
                            Some(&display_name),
                            &roles,
                            &text,
                            &[],
                        )
                        .await
                    {
                        error!("[TwitchIRC] process_incoming_message => {e:?}");
                    }
                }
                info!("[TwitchIRC] read loop ended for user_id={}", user_id_str_for_closure);
            } else {
                info!("[TwitchIRC] no rx => user_id={} might be bot-only or unknown reason", user_id_str_for_closure);
            }
        });

        Ok(PlatformRuntimeHandle {
            join_handle,
            platform: "twitch-irc".into(),
            user_id: user_id_str_for_handle,
            twitch_irc_instance: Some(arc_irc),
            vrchat_instance: None,
            discord_instance: None,
        })
    }

    async fn spawn_twitch_eventsub(&self, credential: PlatformCredential) -> Result<PlatformRuntimeHandle, Error> {
        let user_id_str = credential.user_id.to_string();
        let user_id_str_for_handle = user_id_str.clone();
        let user_id_str_for_closure = user_id_str.clone();

        let event_bus = self.event_bus.clone();

        let mut eventsub = TwitchEventSubPlatform::new();
        eventsub.credentials = Some(credential);

        eventsub.set_event_bus(event_bus);

        let join_handle = tokio::spawn(async move {
            match eventsub.start_loop().await {
                Ok(_) => {
                    info!("[TwitchEventSub] start_loop => OK for user_id={}", user_id_str_for_closure);
                }
                Err(e) => {
                    error!("[TwitchEventSub] start_loop error => {:?}", e);
                }
            }
            info!("[TwitchEventSub] Task ended => user_id={}", user_id_str_for_closure);
        });

        Ok(PlatformRuntimeHandle {
            join_handle,
            platform: "twitch-eventsub".into(),
            user_id: user_id_str_for_handle,
            twitch_irc_instance: None,
            vrchat_instance: None,
            discord_instance: None,
        })
    }

    // -------------------------------------------------------------
    // Twitch-IRC helpers
    // -------------------------------------------------------------
    pub async fn join_twitch_irc_channel(&self, account_name: &str, channel: &str) -> Result<(), Error> {
        let user = self.user_svc.find_user_by_global_username(account_name).await?;
        let key = ("twitch-irc".to_string(), user.user_id.to_string());

        let guard = self.active_runtimes.lock().await;
        let handle_opt = guard.get(&key);
        if let Some(handle) = handle_opt {
            if let Some(irc_arc) = &handle.twitch_irc_instance {
                let irc_lock = irc_arc.lock().await;
                irc_lock.join_channel(channel).await?;
                Ok(())
            } else {
                Err(Error::Platform(format!(
                    "No TwitchIrcPlatform instance found for account='{account_name}'"
                )))
            }
        } else {
            Err(Error::Platform(format!(
                "No active twitch-irc runtime for account='{account_name}'"
            )))
        }
    }

    pub async fn part_twitch_irc_channel(&self, account_name: &str, channel: &str) -> Result<(), Error> {
        let user = self.user_svc.find_user_by_global_username(account_name).await?;
        let key = ("twitch-irc".to_string(), user.user_id.to_string());

        let guard = self.active_runtimes.lock().await;
        let handle_opt = guard.get(&key);
        if let Some(handle) = handle_opt {
            if let Some(irc_arc) = &handle.twitch_irc_instance {
                let irc_lock = irc_arc.lock().await;
                irc_lock.leave_channel(channel).await?;
                Ok(())
            } else {
                Err(Error::Platform(format!(
                    "No TwitchIrcPlatform instance found for account='{account_name}'"
                )))
            }
        } else {
            Err(Error::Platform(format!(
                "No active twitch-irc runtime for account='{account_name}'"
            )))
        }
    }

    pub async fn is_twitch_irc_connected(&self, account_name: &str) -> bool {
        if let Ok(user) = self.user_svc.find_user_by_global_username(account_name).await {
            let key = ("twitch-irc".to_string(), user.user_id.to_string());
            
            let guard = self.active_runtimes.lock().await;
            if let Some(handle) = guard.get(&key) {
                if let Some(irc_arc) = &handle.twitch_irc_instance {
                    let irc_lock = irc_arc.lock().await;
                    // Check if client exists and connection status is Connected
                    return irc_lock.client.is_some() && 
                           matches!(irc_lock.connection_status, ConnectionStatus::Connected);
                }
            }
        }
        false
    }

    pub async fn send_twitch_irc_message(&self, account_name: &str, channel: &str, text: &str) -> Result<(), Error> {
        let user = self.user_svc.find_user_by_global_username(account_name).await?;
        let key = ("twitch-irc".to_string(), user.user_id.to_string());

        let guard = self.active_runtimes.lock().await;
        let handle_opt = guard.get(&key);
        if let Some(handle) = handle_opt {
            if let Some(irc_arc) = &handle.twitch_irc_instance {
                let irc_lock = irc_arc.lock().await;
                irc_lock.send_message(channel, text).await?;
                Ok(())
            } else {
                Err(Error::Platform(format!(
                    "No TwitchIrcPlatform instance found for account='{account_name}'"
                )))
            }
        } else {
            Err(Error::Platform(format!(
                "No active twitch-irc runtime for account='{account_name}'"
            )))
        }
    }

    pub async fn timeout_twitch_user(
        &self,
        _account_name: &str,                 // kept for API parity – no longer used
        channel:       &str,                 // e.g. "#kittyn"
        target_user:   &str,                 // login name
        seconds:       u32,                  // 0 = perm‑ban, else timeout
        reason:        Option<&str>,
    ) -> Result<(), Error> {
        // --- 1. Grab broadcaster credential (must include `moderator:manage:banned_users`). ---
        let cred = self.credentials_repo
            .get_broadcaster_credential(&maowbot_common::models::platform::Platform::Twitch)
            .await?
            .ok_or_else(|| Error::Platform("No broadcaster Twitch credential found".into()))?;

        let client_id = cred
            .additional_data
            .as_ref()
            .and_then(|d| d.get("client_id").and_then(|v| v.as_str()))
            .ok_or_else(|| Error::Platform("Broadcaster credential missing client_id".into()))?;

        let broadcaster_id = cred
            .platform_id
            .clone()
            .ok_or_else(|| Error::Platform("Broadcaster credential missing platform_id".into()))?;

        let helix = crate::platforms::twitch::client::TwitchHelixClient::new(
            &cred.primary_token,
            client_id,
        );

        // --- 2. Resolve user‑id of the target login. ---
        let user_id = helix
            .fetch_user_id(target_user)
            .await?
            .ok_or_else(|| Error::Platform(format!("Unknown Twitch login: {target_user}")))?;

        // --- 3. Issue the ban / timeout. Moderator = broadcaster for simplicity. ---
        helix
            .ban_user(
                &broadcaster_id,
                &broadcaster_id,                 // moderator = broadcaster
                &user_id,
                if seconds == 0 { None } else { Some(seconds) },
                reason,
            )
            .await
    }
    // -------------------------------------------------------------
    // NEW HELPER: Having each TTV-IRC instance join channels
    // of all other Twitch-IRC credentials.
    //
    // The user’s request: “All twitch-irc accounts that start
    // up join all other twitch accounts’ chats.”
    //
    // Implementation: we gather *all* credentials for platform
    // twitch-irc, ignoring the newly started one’s own user_id,
    // and call join_channel for each credential’s user_name.
    //
    // NOTE: We typically do “join_channel(#NAME)” but that
    // depends on how your code expects the channel string.
    // This example calls “join_channel” with `#` prefix if
    // the code typically expects that.
    // -------------------------------------------------------------
    async fn join_all_twitch_channels(
        &self,
        irc_platform: &TwitchIrcPlatform,
        my_user_id: uuid::Uuid,
    ) -> Result<(), Error> {
        let all_irc_creds = self.credentials_repo
            .list_credentials_for_platform(&Platform::TwitchIRC)
            .await?;
        for c in all_irc_creds {
            if c.user_id == my_user_id {
                continue;
            }
            let channel_name = format!("#{}", c.user_name);
            if let Err(e) = irc_platform.join_channel(&channel_name).await {
                warn!("join_channel('{}') error => {:?}", channel_name, e);
            }
        }
        Ok(())
    }

    // -------------------------------------------------------------
    // VRChat helper
    // -------------------------------------------------------------
    pub async fn get_vrchat_instance(&self, account_name: &str) -> Result<Arc<AsyncMutex<VRChatPlatform>>, Error> {
        let user = self.user_svc.find_user_by_global_username(account_name).await?;
        let key = ("vrchat".to_string(), user.user_id.to_string());
        let guard = self.active_runtimes.lock().await;
        let handle_opt = guard.get(&key);
        if let Some(handle) = handle_opt {
            if let Some(vrc_arc) = &handle.vrchat_instance {
                Ok(Arc::clone(vrc_arc))
            } else {
                Err(Error::Platform(format!(
                    "No VRChatPlatform instance found for account='{account_name}'"
                )))
            }
        } else {
            Err(Error::Platform(format!(
                "No active VRChat runtime for account='{account_name}'"
            )))
        }
    }

    /// Find Discord channel ID by channel name for given guild
    pub async fn find_discord_channel_id(
        &self,
        account_name: &str,
        guild_id: &str, 
        channel_name: &str
    ) -> Result<Option<String>, Error> {
        // Try to get cache
        let cache = self.get_discord_cache(account_name).await?;
        
        debug!("Looking for channel '{}' in guild_id: '{}'", channel_name, guild_id);
        
        // First search with guild context if available
        if !guild_id.is_empty() {
            debug!("Searching for channel with guild context");
            for channel_ref in cache.iter().channels() {
                if let Some(name) = &channel_ref.value().name {
                    if name.eq_ignore_ascii_case(channel_name) {
                        if let Some(channel_guild_id) = channel_ref.value().guild_id {
                            // Parse the guild_id
                            if let Ok(guild_id_u64) = guild_id.parse::<u64>() {
                                let guild_id_twilight = twilight_model::id::Id::new(guild_id_u64);
                                if channel_guild_id == guild_id_twilight {
                                    debug!("Found channel '{}' (ID: {}) in specified guild", 
                                           name, channel_ref.key());
                                    return Ok(Some(channel_ref.key().to_string()));
                                }
                            }
                        }
                    }
                }
            }
        }
        
        // If we couldn't find a match with guild context, or guild_id is empty,
        // try searching across all guilds (less accurate but works as fallback)
        debug!("Falling back to searching all guilds for channel '{}'", channel_name);
        for channel_ref in cache.iter().channels() {
            if let Some(name) = &channel_ref.value().name {
                if name.eq_ignore_ascii_case(channel_name) {
                    debug!("Found channel '{}' (ID: {}) in some guild", 
                           name, channel_ref.key());
                    return Ok(Some(channel_ref.key().to_string()));
                }
            }
        }
        
        debug!("No channel found with name '{}'", channel_name);
        // No matching channel found
        Ok(None)
    }
    
    pub async fn send_discord_message(
        &self,
        account_name: &str,
        server_id: &str,
        channel_id_or_name: &str,
        text: &str
    ) -> Result<(), Error> {
        let user = self.user_svc.find_user_by_global_username(account_name).await?;
        let key = ("discord".to_string(), user.user_id.to_string());
        
        // Check if the channel needs to be resolved from a name to an ID
        let channel_id = if !channel_id_or_name.chars().all(|c| c.is_ascii_digit()) {
            // Not all digits, so probably a channel name
            debug!("Channel '{}' is not numeric, attempting to find ID", channel_id_or_name);
            
            if let Some(id) = self.find_discord_channel_id(account_name, server_id, channel_id_or_name).await? {
                debug!("Resolved channel name '{}' to ID '{}'", channel_id_or_name, id);
                id
            } else {
                return Err(Error::Platform(format!("Could not find Discord channel with name: {}", channel_id_or_name)));
            }
        } else {
            // Already an ID
            channel_id_or_name.to_string()
        };

        let guard = self.active_runtimes.lock().await;
        if let Some(handle) = guard.get(&key) {
            if let Some(discord_arc) = &handle.discord_instance {
                let discord_lock = discord_arc;
                discord_lock.send_message(&channel_id, text).await
            } else {
                Err(Error::Platform(format!(
                    "No DiscordPlatform instance found for account='{account_name}'"
                )))
            }
        } else {
            Err(Error::Platform(format!(
                "No active Discord runtime for account='{account_name}'"
            )))
        }
    }
    pub async fn add_role_to_discord_user(
        &self,
        account_name: &str,
        guild_id: &str,
        user_id: &str,
        role_id: &str
    ) -> Result<(), Error> {
        // Get the Discord instance
        let discord = self.get_discord_instance(account_name).await?;
        
        // Parse the guild ID
        let guild_id_u64 = guild_id.parse::<u64>()
            .map_err(|_| Error::Platform(format!("Invalid guild ID: {}", guild_id)))?;
            
        // Parse the user ID
        let user_id_u64 = user_id.parse::<u64>()
            .map_err(|_| Error::Platform(format!("Invalid user ID: {}", user_id)))?;
            
        // Parse the role ID
        let role_id_u64 = role_id.parse::<u64>()
            .map_err(|_| Error::Platform(format!("Invalid role ID: {}", role_id)))?;
            
        // Create Twilight ID objects
        let guild_id = twilight_model::id::Id::<twilight_model::id::marker::GuildMarker>::new(guild_id_u64);
        let user_id = twilight_model::id::Id::<twilight_model::id::marker::UserMarker>::new(user_id_u64);
        let role_id = twilight_model::id::Id::<twilight_model::id::marker::RoleMarker>::new(role_id_u64);
        
        // Call the API to add the role
        if let Some(http) = &discord.http {
            http.add_guild_member_role(guild_id, user_id, role_id)
                .await
                .map_err(|e| Error::Platform(format!("Failed to add role to user: {}", e)))?;
        } else {
            return Err(Error::Platform("Discord HTTP client not initialized".into()));
        }
        
        Ok(())
    }
    
    pub async fn remove_role_from_discord_user(
        &self,
        account_name: &str,
        guild_id: &str,
        user_id: &str,
        role_id: &str
    ) -> Result<(), Error> {
        // Get the Discord instance
        let discord = self.get_discord_instance(account_name).await?;
        
        // Parse the guild ID
        let guild_id_u64 = guild_id.parse::<u64>()
            .map_err(|_| Error::Platform(format!("Invalid guild ID: {}", guild_id)))?;
            
        // Parse the user ID
        let user_id_u64 = user_id.parse::<u64>()
            .map_err(|_| Error::Platform(format!("Invalid user ID: {}", user_id)))?;
            
        // Parse the role ID
        let role_id_u64 = role_id.parse::<u64>()
            .map_err(|_| Error::Platform(format!("Invalid role ID: {}", role_id)))?;
            
        // Create Twilight ID objects
        let guild_id = twilight_model::id::Id::<twilight_model::id::marker::GuildMarker>::new(guild_id_u64);
        let user_id = twilight_model::id::Id::<twilight_model::id::marker::UserMarker>::new(user_id_u64);
        let role_id = twilight_model::id::Id::<twilight_model::id::marker::RoleMarker>::new(role_id_u64);
        
        // Call the API to remove the role
        if let Some(http) = &discord.http {
            http.remove_guild_member_role(guild_id, user_id, role_id)
                .await
                .map_err(|e| Error::Platform(format!("Failed to remove role from user: {}", e)))?;
        } else {
            return Err(Error::Platform("Discord HTTP client not initialized".into()));
        }
        
        Ok(())
    }

    pub async fn send_discord_embed(
        &self,
        account_name: &str,
        server_id: &str,
        channel_id_or_name: &str,
        embed: &DiscordEmbed,
        content: Option<&str>
    ) -> Result<(), Error> {
        // Check if the channel needs to be resolved from a name to an ID
        let channel_id = if !channel_id_or_name.chars().all(|c| c.is_ascii_digit()) {
            // Not all digits, so probably a channel name
            debug!("Channel '{}' is not numeric, attempting to find ID for embed", channel_id_or_name);
            
            if let Some(id) = self.find_discord_channel_id(account_name, server_id, channel_id_or_name).await? {
                debug!("Resolved channel name '{}' to ID '{}'", channel_id_or_name, id);
                id
            } else {
                return Err(Error::Platform(format!("Could not find Discord channel with name: {}", channel_id_or_name)));
            }
        } else {
            // Already an ID
            channel_id_or_name.to_string()
        };
        
        // Get the Discord platform for the specified account
        let discord = self.get_discord_instance(account_name).await?;

        // Send the embed to the channel
        discord.send_channel_embed(&channel_id, embed, content).await
    }
}
