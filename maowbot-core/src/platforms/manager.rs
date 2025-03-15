use std::collections::HashMap;
use std::sync::Arc;
use tokio::task::JoinHandle;
use tracing::{info, error, warn};
use tokio::sync::Mutex as AsyncMutex;
use std::sync::Mutex;

use maowbot_common::models::platform::{Platform, PlatformCredential};
use maowbot_common::traits::platform_traits::{ChatPlatform, ConnectionStatus, PlatformIntegration};
use maowbot_common::traits::repository_traits::CredentialsRepository;
use crate::eventbus::EventBus;
use crate::services::message_service::MessageService;
use crate::services::user_service::UserService;
use crate::Error;

use crate::platforms::discord::runtime::DiscordPlatform;
use crate::platforms::twitch::runtime::TwitchPlatform;
use crate::platforms::vrchat_pipeline::runtime::VRChatPlatform;
use crate::platforms::twitch_irc::runtime::TwitchIrcPlatform;
use crate::platforms::twitch_eventsub::runtime::TwitchEventSubPlatform;

pub struct PlatformRuntimeHandle {
    pub join_handle: JoinHandle<()>,
    pub platform: String,
    pub user_id: String,

    pub twitch_irc_instance: Option<Arc<AsyncMutex<TwitchIrcPlatform>>>,
    pub vrchat_instance: Option<Arc<AsyncMutex<VRChatPlatform>>>,
    pub discord_instance: Option<Arc<AsyncMutex<DiscordPlatform>>>,
}

/// Manages starting/stopping platform runtimes, holding references to them, etc.
pub struct PlatformManager {
    message_service: Mutex<Option<Arc<MessageService>>>,
    user_svc: Arc<UserService>,
    event_bus: Arc<EventBus>,
    pub(crate) credentials_repo: Arc<dyn CredentialsRepository + Send + Sync>,

    pub active_runtimes: AsyncMutex<HashMap<(String, String), PlatformRuntimeHandle>>,
}

impl PlatformManager {
    pub fn new(
        user_svc: Arc<UserService>,
        event_bus: Arc<EventBus>,
        credentials_repo: Arc<dyn CredentialsRepository + Send + Sync>,
    ) -> Self {
        Self {
            message_service: Mutex::new(None),
            user_svc,
            event_bus,
            credentials_repo,
            active_runtimes: AsyncMutex::new(HashMap::new()),
        }
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

    // -----------------------------------------------------------------------
    // spawn_* methods
    // -----------------------------------------------------------------------
    async fn spawn_discord(&self, credential: PlatformCredential) -> Result<PlatformRuntimeHandle, Error> {
        let message_svc = self.get_message_service()?;

        let user_id_str = credential.user_id.to_string();
        let user_id_str_for_handle = user_id_str.clone();
        let user_id_str_for_closure = user_id_str.clone();

        let token = credential.primary_token.clone();

        let mut discord = DiscordPlatform::new(token);
        discord.set_event_bus(self.event_bus.clone());
        discord.connect().await?;
        info!("[Discord] Connected for user_id={}", user_id_str_for_closure);

        let arc_discord = Arc::new(AsyncMutex::new(discord));
        let cloned_discord = arc_discord.clone();

        let join_handle = tokio::spawn(async move {
            loop {
                let msg_event_opt = cloned_discord.lock().await.next_message_event().await;
                match msg_event_opt {
                    Some(msg_event) => {
                        let channel = msg_event.channel;
                        let user_platform_id = msg_event.user_id.clone();
                        let display_name = msg_event.username.clone();
                        let roles = msg_event.user_roles.clone();
                        let text = msg_event.text;

                        if let Err(e) = message_svc
                            .process_incoming_message(
                                "discord",
                                &channel,
                                &user_platform_id,
                                Some(&display_name),
                                &roles,
                                &text,
                            )
                            .await
                        {
                            error!("[Discord] process_incoming_message => {e:?}");
                        }
                    }
                    None => {
                        info!("[Discord] No more events => user_id={}", user_id_str_for_closure);
                        break;
                    }
                }
            }
        });

        Ok(PlatformRuntimeHandle {
            join_handle,
            platform: "discord".into(),
            user_id: user_id_str_for_handle,
            twitch_irc_instance: None,
            vrchat_instance: None,
            discord_instance: Some(arc_discord),
        })
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

    // -------------------------------------------------------------
    // NEW: Send a message to Discord from a given account + server + channel
    // -------------------------------------------------------------
    pub async fn send_discord_message(
        &self,
        account_name: &str,
        server_id: &str,
        channel_id: &str,
        text: &str
    ) -> Result<(), Error> {
        let user = self.user_svc.find_user_by_global_username(account_name).await?;
        let key = ("discord".to_string(), user.user_id.to_string());

        let guard = self.active_runtimes.lock().await;
        if let Some(handle) = guard.get(&key) {
            if let Some(discord_arc) = &handle.discord_instance {
                let discord_lock = discord_arc.lock().await;
                discord_lock.send_message(channel_id, text).await
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
}
