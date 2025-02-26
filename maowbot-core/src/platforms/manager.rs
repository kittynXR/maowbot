use std::collections::HashMap;
use std::sync::Arc;
use tokio::task::JoinHandle;
use tracing::{info, error, warn};
use tokio::sync::Mutex;

use crate::eventbus::EventBus;
use crate::services::message_service::MessageService;
use crate::services::user_service::UserService;
use crate::Error;
use crate::models::{PlatformCredential, Platform};
use crate::repositories::postgres::credentials::CredentialsRepository;

use crate::platforms::discord::runtime::DiscordPlatform;
use crate::platforms::{ChatPlatform, PlatformIntegration};
use crate::platforms::twitch::runtime::TwitchPlatform;
use crate::platforms::vrchat_pipeline::runtime::VRChatPlatform;
use crate::platforms::twitch_irc::runtime::TwitchIrcPlatform;
use crate::platforms::twitch_eventsub::runtime::TwitchEventSubPlatform;

/// Holds the JoinHandle for a running platform plus references
/// to specialized objects (e.g. TwitchIrcPlatform instance).
pub struct PlatformRuntimeHandle {
    pub join_handle: JoinHandle<()>,
    pub platform: String,
    pub user_id: String,

    /// For Twitch IRC subcommands
    pub twitch_irc_instance: Option<Arc<Mutex<TwitchIrcPlatform>>>,

    /// For VRChat commands
    pub vrchat_instance: Option<Arc<Mutex<VRChatPlatform>>>,
}

pub struct PlatformManager {
    message_svc: Arc<MessageService>,
    user_svc: Arc<UserService>,
    event_bus: Arc<EventBus>,
    credentials_repo: Arc<dyn CredentialsRepository + Send + Sync>,

    /// (platform, user_id) => runtime handle
    pub active_runtimes: tokio::sync::Mutex<HashMap<(String, String), PlatformRuntimeHandle>>,
}

impl PlatformManager {
    pub fn new(
        message_svc: Arc<MessageService>,
        user_svc: Arc<UserService>,
        event_bus: Arc<EventBus>,
        credentials_repo: Arc<dyn CredentialsRepository + Send + Sync>,
    ) -> Self {
        Self {
            message_svc,
            user_svc,
            event_bus,
            credentials_repo,
            active_runtimes: tokio::sync::Mutex::new(HashMap::new()),
        }
    }

    /// Start a platform runtime (Discord, Twitch, etc.) given the platform string and account_name.
    /// For multi-account usage, the combination of (platform, user_id) is unique.
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

        // See if already running
        let key = (platform_str.to_string(), user.user_id.to_string());
        {
            let guard = self.active_runtimes.lock().await;
            if guard.contains_key(&key) {
                info!(
                    "Runtime already running for platform='{platform_str}' user_id='{}'. Skipping.",
                    user.user_id
                );
                return Ok(());
            }
        }

        // spawn
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

    /// Stop a running platform runtime if found.
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
            // If we need a graceful close, do it now or simply abort:
            rh.join_handle.abort();
            info!("Stopped runtime for platform='{platform_str}', user_id={}", user.user_id);
        } else {
            warn!("No active runtime for platform='{platform_str}', account='{account_name}'");
        }
        Ok(())
    }

    // -------------------------------------------------------------
    // spawn_* methods
    // -------------------------------------------------------------

    async fn spawn_discord(&self, credential: PlatformCredential) -> Result<PlatformRuntimeHandle, Error> {
        let token = credential.primary_token.clone();
        let message_svc = Arc::clone(&self.message_svc);

        let platform_str = "discord".to_string();
        let user_id_str = credential.user_id.to_string();

        let user_id_str_cloned = user_id_str.clone();

        let mut discord = DiscordPlatform::new(token);
        discord.set_event_bus(self.event_bus.clone());
        discord.connect().await?;
        info!("[Discord] Connected for user_id={user_id_str_cloned}");

        let join_handle = tokio::spawn(async move {
            while let Some(msg_event) = discord.next_message_event().await {
                let channel = msg_event.channel;
                let user_platform_id = msg_event.user_id.clone(); // numeric ID from Discord
                let display_name = msg_event.username.clone();     // user’s server nickname or username
                let roles = msg_event.user_roles.clone();
                let text = msg_event.text;

                // NOTE: we do *not* call event_bus.publish_chat(...) ourselves.
                // Instead, we rely on the message_service to do final publishing.
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

            info!("[Discord] Task ended for user_id={user_id_str_cloned}");
        });

        Ok(PlatformRuntimeHandle {
            join_handle,
            platform: platform_str,
            user_id: user_id_str,
            twitch_irc_instance: None,
            vrchat_instance: None,
        })
    }

    async fn spawn_twitch_helix(&self, credential: PlatformCredential) -> Result<PlatformRuntimeHandle, Error> {
        let message_svc = Arc::clone(&self.message_svc);

        let platform_str = "twitch".to_string();
        let user_id_str = credential.user_id.to_string();
        let user_id_str_cloned = user_id_str.clone();

        let join_handle = tokio::spawn(async move {
            let mut twitch = TwitchPlatform {
                credentials: Some(credential.clone()),
                connection_status: crate::platforms::ConnectionStatus::Disconnected,
                client: None,
            };
            if let Err(err) = twitch.connect().await {
                error!("[TwitchHelix] connect error: {err:?}");
                return;
            }
            info!("[TwitchHelix] Connected for user_id={user_id_str_cloned}");

            while let Some(msg_event) = twitch.next_message_event().await {
                let channel = msg_event.channel;
                // Helix events can include user_id or display_name. For demonstration:
                let text = msg_event.text;
                let display_name = msg_event.display_name.clone();
                // We do *not* have a numeric user ID from Helix chat events the same way IRC does,
                // so let's put an empty string or fallback.
                let platform_user_id = msg_event.user_id; // If provided by your Helix logic

                // We'll call the message_service:
                if let Err(e) = message_svc
                    .process_incoming_message(
                        "twitch",
                        &channel,
                        &platform_user_id, // e.g. "264653338" if you have it
                        Some(&display_name),
                        &[], // no roles from Helix for now
                        &text,
                    )
                    .await
                {
                    error!("[TwitchHelix] process_incoming_message => {e:?}");
                }
            }

            info!("[TwitchHelix] Task ended for user_id={user_id_str_cloned}");
        });

        Ok(PlatformRuntimeHandle {
            join_handle,
            platform: platform_str,
            user_id: user_id_str,
            twitch_irc_instance: None,
            vrchat_instance: None,
        })
    }

    async fn spawn_vrchat(&self, credential: PlatformCredential) -> Result<PlatformRuntimeHandle, Error> {
        let message_svc = Arc::clone(&self.message_svc);

        let platform_str = "vrchat".to_string();
        let user_id_str = credential.user_id.to_string();
        let user_id_str_cloned = user_id_str.clone();

        let mut vrc = VRChatPlatform::new();
        vrc.credentials = Some(credential);

        let arc_vrc = Arc::new(Mutex::new(vrc));

        let cloned_vrc = Arc::clone(&arc_vrc);
        let join_handle = tokio::spawn(async move {
            let mut lock = cloned_vrc.lock().await;
            if let Err(err) = lock.connect().await {
                error!("[VRChat] connect error: {err:?}");
                return;
            }
            info!("[VRChat] Connected for user_id={user_id_str_cloned}");

            while let Some(evt) = lock.next_message_event().await {
                let channel = "roomOrWorldId"; // or wherever the event came from
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

            info!("[VRChat] Task ended for user_id={user_id_str_cloned}");
        });

        Ok(PlatformRuntimeHandle {
            join_handle,
            platform: platform_str,
            user_id: user_id_str,
            twitch_irc_instance: None,
            vrchat_instance: Some(arc_vrc),
        })
    }

    async fn spawn_twitch_irc(&self, credential: PlatformCredential) -> Result<PlatformRuntimeHandle, Error> {
        let message_svc = Arc::clone(&self.message_svc);

        let user_id_str = credential.user_id.to_string();
        let user_id_str_cloned = user_id_str.clone();

        let mut irc = TwitchIrcPlatform::new();
        irc.set_credentials(credential);

        irc.set_event_bus(self.event_bus.clone());
        irc.connect().await?;
        info!("[TwitchIRC] connected ... starting read loop");

        let rx_opt = irc.rx.take();
        let rx = match rx_opt {
            Some(r) => r,
            None => return Err(Error::Platform("No IRC receiver found".into())),
        };
        let arc_irc = Arc::new(Mutex::new(irc));

        let join_handle = tokio::spawn(async move {
            let mut msg_rx = rx;
            while let Some(evt) = msg_rx.recv().await {
                let channel = evt.channel;
                let platform_user_id = evt.twitch_user_id.clone(); // numeric user-id from tags
                let display_name = evt.display_name.clone();       // e.g. "kittyncat"
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
            info!("[TwitchIRC] read loop ended for user_id={user_id_str_cloned}");
        });

        Ok(PlatformRuntimeHandle {
            join_handle,
            platform: "twitch-irc".into(),
            user_id: user_id_str,
            twitch_irc_instance: Some(arc_irc),
            vrchat_instance: None,
        })
    }

    async fn spawn_twitch_eventsub(&self, credential: PlatformCredential) -> Result<PlatformRuntimeHandle, Error> {
        let event_bus = self.event_bus.clone();
        let user_id_str = credential.user_id.to_string();
        let user_id_str_cloned = user_id_str.clone();

        let mut eventsub = TwitchEventSubPlatform::new();
        eventsub.credentials = Some(credential);
        eventsub.set_event_bus(event_bus);

        let join_handle = tokio::spawn(async move {
            match eventsub.start_loop().await {
                Ok(_) => {
                    info!("[TwitchEventSub] start_loop returned OK for user_id={user_id_str_cloned}");
                }
                Err(e) => {
                    error!("[TwitchEventSub] start_loop error => {:?}", e);
                }
            }
            info!("[TwitchEventSub] Task ended for user_id={user_id_str_cloned}");
        });

        Ok(PlatformRuntimeHandle {
            join_handle,
            platform: "twitch-eventsub".to_string(),
            user_id: user_id_str,
            twitch_irc_instance: None,
            vrchat_instance: None,
        })
    }

    // -------------------------------------------------------------
    // Twitch IRC helper subcommands (join/part/say)
    // -------------------------------------------------------------
    pub async fn join_twitch_irc_channel(&self, account_name: &str, channel: &str) -> Result<(), Error> {
        let user = self.user_svc.find_user_by_global_username(account_name).await?;
        let key = ("twitch-irc".to_string(), user.user_id.to_string());

        let guard = self.active_runtimes.lock().await;
        let handle_opt = guard.get(&key);
        if let Some(handle) = handle_opt {
            if let Some(irc_arc) = &handle.twitch_irc_instance {
                let mut irc_lock = irc_arc.lock().await;
                irc_lock.join_channel(channel).await?;
                Ok(())
            } else {
                Err(Error::Platform(format!(
                    "No TwitchIrcPlatform instance found for account='{account_name}'"
                )))
            }
        } else {
            Err(Error::Platform(format!(
                "No active twitch-irc runtime for account='{account_name}'? \
                 Did you run 'start twitch-irc {account_name}'?"
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
                let mut irc_lock = irc_arc.lock().await;
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
                let mut irc_lock = irc_arc.lock().await;
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
    // VRChat helper methods
    // -------------------------------------------------------------
    pub async fn get_vrchat_instance(&self, account_name: &str) -> Result<Arc<Mutex<VRChatPlatform>>, Error> {
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
}
