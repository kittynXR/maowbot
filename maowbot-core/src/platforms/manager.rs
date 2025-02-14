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

// Runtimes
use crate::platforms::discord::runtime::DiscordPlatform;
use crate::platforms::{ChatPlatform, PlatformIntegration};
use crate::platforms::twitch_helix::runtime::TwitchPlatform;
use crate::platforms::vrchat::runtime::VRChatPlatform;
use crate::platforms::twitch_irc::runtime::TwitchIrcPlatform;
use crate::platforms::twitch_eventsub::runtime::TwitchEventSubPlatform;

/// We extend `PlatformRuntimeHandle` with an optional reference to
/// the underlying `TwitchIrcPlatform` instance (or other platforms in the future).
///
/// For `Twitch IRC`, we’ll store `Some(Arc<Mutex<TwitchIrcPlatform>>>)`.
/// For everything else, it remains `None`.
pub struct PlatformRuntimeHandle {
    pub join_handle: JoinHandle<()>,
    pub platform: String,
    pub user_id: String,

    /// Only populated for Twitch IRC. Other platforms remain `None`.
    pub twitch_irc_instance: Option<Arc<Mutex<TwitchIrcPlatform>>>,
}

/// Manages multiple concurrent platform runtimes.
/// Key = (platform, user_id).
///
/// - For each active runtime, we store a `PlatformRuntimeHandle` so we can stop it on demand.
/// - Twitch IRC subcommands (join/part/message) require a direct reference to the underlying
///   `TwitchIrcPlatform` instance, so that instance is stored in the handle.
pub struct PlatformManager {
    message_svc: Arc<MessageService>,
    user_svc: Arc<UserService>,
    event_bus: Arc<EventBus>,
    credentials_repo: Arc<dyn CredentialsRepository + Send + Sync>,

    /// A HashMap tracking each running platform instance by (platform, user_id).
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

    /// Called to start a runtime for (platform, account_name).
    /// If already started, does nothing.
    pub async fn start_platform_runtime(
        &self,
        platform_str: &str,
        account_name: &str,
    ) -> Result<(), Error> {
        // 1) find user by global username
        let user = self.user_svc
            .find_user_by_global_username(account_name)
            .await?;

        let platform = platform_str.parse::<Platform>()
            .map_err(|_| Error::Platform(format!("Unknown platform '{}'", platform_str)))?;

        // 2) get credentials from DB
        let creds_opt = self.credentials_repo
            .get_credentials(&platform, user.user_id)
            .await?;
        let creds = match creds_opt {
            Some(c) => c,
            None => {
                return Err(Error::Auth(format!(
                    "No credentials for user='{}' and platform='{}'",
                    account_name, platform_str
                )));
            }
        };

        // 3) check if already started
        let key = (platform_str.to_string(), user.user_id.to_string());
        {
            let guard = self.active_runtimes.lock().await;
            if guard.contains_key(&key) {
                info!(
                    "Runtime already running for platform='{}' user_id='{}'. Skipping.",
                    platform_str, user.user_id
                );
                return Ok(());
            }
        }

        // 4) spawn the runtime
        let handle = match platform {
            Platform::Discord => self.spawn_discord(creds).await?,
            Platform::Twitch => self.spawn_twitch_helix(creds).await?,
            Platform::VRChat => self.spawn_vrchat(creds).await?,
            Platform::TwitchIRC => self.spawn_twitch_irc(creds).await?,
            Platform::TwitchEventSub => self.spawn_twitch_eventsub(creds).await?,
        };

        // 5) store handle
        {
            let mut guard = self.active_runtimes.lock().await;
            guard.insert(key, handle);
        }

        Ok(())
    }

    /// Stops a runtime for (platform, account_name).
    pub async fn stop_platform_runtime(
        &self,
        platform_str: &str,
        account_name: &str,
    ) -> Result<(), Error> {
        let user = self.user_svc
            .find_user_by_global_username(account_name)
            .await?;

        let key = (platform_str.to_string(), user.user_id.to_string());

        // remove from map & abort
        let handle_opt = {
            let mut guard = self.active_runtimes.lock().await;
            guard.remove(&key)
        };
        if let Some(rh) = handle_opt {
            rh.join_handle.abort();
            info!("Stopped runtime for platform='{}', user_id={}", platform_str, user.user_id);
        } else {
            warn!(
                "No active runtime found for platform='{}', account='{}'",
                platform_str, account_name
            );
        }
        Ok(())
    }

    // --------------------------------------------------------
    // The spawn_* methods each return a PlatformRuntimeHandle
    // --------------------------------------------------------

    async fn spawn_discord(
        &self,
        credential: PlatformCredential
    ) -> Result<PlatformRuntimeHandle, Error> {
        let token = credential.primary_token.clone();
        let message_svc = Arc::clone(&self.message_svc);
        let user_svc = Arc::clone(&self.user_svc);

        let platform_str = "discord".to_string();
        let plat = platform_str.clone();
        let user_id_str = credential.user_id.to_string();
        let user_id_str_clone = user_id_str.clone();

        let join_handle = tokio::spawn(async move {
            let mut discord = DiscordPlatform::new(token);
            if let Err(err) = discord.connect().await {
                error!("[Discord] connect error: {:?}", err);
                return;
            }
            info!("[Discord] Connected for user_id={}", user_id_str_clone);

            while let Some(msg_event) = discord.next_message_event().await {
                let channel = msg_event.channel;
                let user_platform_id = msg_event.user_id; // ephemeral ID from platform
                let text = msg_event.text;
                let username = msg_event.username.clone();

                // 1) get/create user in DB
                let user = match user_svc
                    .get_or_create_user("discord", &user_platform_id, Some(&username))
                    .await
                {
                    Ok(u) => u,
                    Err(e) => {
                        error!("[Discord] user_svc error: {:?}", e);
                        continue;
                    }
                };

                // 2) pass ephemeral username to message_svc (not the DB user_id)
                if let Err(e) = message_svc
                    .process_incoming_message(&platform_str, &channel, &username, &text)
                    .await
                {
                    error!("[Discord] process_incoming_message failed: {:?}", e);
                }
            }

            info!("[Discord] Task ended for user_id={}", user_id_str_clone);
        });

        let rh = PlatformRuntimeHandle {
            join_handle,
            platform: plat,
            user_id: user_id_str,
            twitch_irc_instance: None,
        };
        Ok(rh)
    }

    async fn spawn_twitch_helix(
        &self,
        credential: PlatformCredential
    ) -> Result<PlatformRuntimeHandle, Error> {
        let message_svc = Arc::clone(&self.message_svc);
        let user_svc = Arc::clone(&self.user_svc);

        let platform_str = "twitch".to_string();
        let plat = platform_str.clone();
        let user_id_str = credential.user_id.to_string();
        let user_id_str_clone = user_id_str.clone();

        let join_handle = tokio::spawn(async move {
            let mut twitch = TwitchPlatform {
                credentials: Some(credential.clone()),
                connection_status: crate::platforms::ConnectionStatus::Disconnected,
                client: None,
            };
            if let Err(err) = twitch.connect().await {
                error!("[TwitchHelix] connect error: {:?}", err);
                return;
            }
            info!("[TwitchHelix] Connected for user_id={}", user_id_str_clone);

            while let Some(msg_event) = twitch.next_message_event().await {
                let channel = msg_event.channel;
                let user_platform_id = msg_event.user_id;
                let text = msg_event.text;
                let display_name = msg_event.display_name.clone();

                let user = match user_svc
                    .get_or_create_user("twitch", &user_platform_id, Some(&display_name))
                    .await
                {
                    Ok(u) => u,
                    Err(e) => {
                        error!("[TwitchHelix] user_svc error: {:?}", e);
                        continue;
                    }
                };

                if let Err(e) = message_svc
                    .process_incoming_message(&platform_str, &channel, &display_name, &text)
                    .await
                {
                    error!("[TwitchHelix] process_incoming_message failed: {:?}", e);
                }
            }

            info!("[TwitchHelix] Task ended for user_id={}", user_id_str_clone);
        });

        Ok(PlatformRuntimeHandle {
            join_handle,
            platform: plat,
            user_id: user_id_str,
            twitch_irc_instance: None,
        })
    }

    async fn spawn_vrchat(
        &self,
        credential: PlatformCredential
    ) -> Result<PlatformRuntimeHandle, Error> {
        let message_svc = Arc::clone(&self.message_svc);
        let user_svc = Arc::clone(&self.user_svc);

        let platform_str = "vrchat".to_string();
        let plat = platform_str.clone();
        let user_id_str = credential.user_id.to_string();
        let user_id_str_clone = user_id_str.clone();

        let join_handle = tokio::spawn(async move {
            let mut vrc = VRChatPlatform {
                credentials: Some(credential.clone()),
                connection_status: crate::platforms::ConnectionStatus::Disconnected,
            };
            if let Err(err) = vrc.connect().await {
                error!("[VRChat] connect error: {:?}", err);
                return;
            }
            info!("[VRChat] Connected for user_id={}", user_id_str_clone);

            while let Some(evt) = vrc.next_message_event().await {
                let user_platform_id = evt.user_id;
                let text = evt.text;
                let display_name = evt.vrchat_display_name.clone();

                let user = match user_svc
                    .get_or_create_user("vrchat", &user_platform_id, Some(&display_name))
                    .await
                {
                    Ok(u) => u,
                    Err(e) => {
                        error!("[VRChat] user_svc error: {:?}", e);
                        continue;
                    }
                };

                // For VRChat, we have no real concept of "channel," so pass something like roomId:
                if let Err(e) = message_svc
                    .process_incoming_message(&platform_str, "roomOrWorldId", &display_name, &text)
                    .await
                {
                    error!("[VRChat] process_incoming_message failed: {:?}", e);
                }
            }

            info!("[VRChat] Task ended for user_id={}", user_id_str_clone);
        });

        Ok(PlatformRuntimeHandle {
            join_handle,
            platform: plat,
            user_id: user_id_str,
            twitch_irc_instance: None,
        })
    }

    async fn spawn_twitch_irc(&self, credential: PlatformCredential) -> Result<PlatformRuntimeHandle, Error> {
        let message_svc = Arc::clone(&self.message_svc);
        let user_svc = Arc::clone(&self.user_svc);

        // 1) create the TwitchIrcPlatform
        let mut irc = TwitchIrcPlatform::new();
        irc.set_credentials(credential.clone());

        // 2) connect it once, storing all needed fields
        irc.connect().await?;
        // That sets up self.rx, self.tx, self.client, etc.

        // 3) Now extract the `rx` from inside the platform,
        //    so the read loop doesn't keep the entire `Mutex`.
        //    We'll keep the rest of the data in `irc`, but the channel is separate.
        let rx_opt = irc.rx.take(); // or some function "irc.take_rx()"
        let rx = match rx_opt {
            Some(r) => r,
            None => {
                return Err(Error::Platform("No IRC receiver found".into()));
            }
        };

        // 4) wrap the *remaining* platform in Arc<Mutex<...>> so we can do join_channel, etc.
        let arc_irc = Arc::new(tokio::sync::Mutex::new(irc));

        // 5) spawn the read loop, which just uses `rx` without locking arc_irc
        let arc_irc_for_task = arc_irc.clone();
        let join_handle = tokio::spawn(async move {
            info!("[TwitchIRC] connected ... starting read loop");
            let mut msg_rx = rx;
            while let Some(evt) = msg_rx.recv().await {
                let channel = evt.channel;
                let user_platform_id = evt.user_id;
                let text = evt.text;
                let user_name = evt.user_name.clone();

                // we only lock the platform if we truly need something from it,
                // e.g. arc_irc_for_task.lock().await.connection_status = ...
                // But typically, we might not need it.

                // proceed with user_svc etc.
                let user = match user_svc.get_or_create_user("twitch-irc", &user_platform_id, Some(&user_name)).await {
                    Ok(u) => u,
                    Err(e) => {
                        error!("[TwitchIRC] user_svc error: {:?}", e);
                        continue;
                    }
                };
                // message_svc
                if let Err(e) = message_svc.process_incoming_message("twitch-irc", &channel, &user_name, &text).await {
                    error!("[TwitchIRC] process_incoming_message => {:?}", e);
                }
            }
            info!("[TwitchIRC] read loop ended for ???");
        });

        // 6) Return the handle
        Ok(PlatformRuntimeHandle {
            join_handle,
            platform: "twitch-irc".to_string(),
            user_id: credential.user_id.to_string(),
            twitch_irc_instance: Some(arc_irc),
        })
    }

    async fn spawn_twitch_eventsub(
        &self,
        credential: PlatformCredential
    ) -> Result<PlatformRuntimeHandle, Error> {
        let message_svc = Arc::clone(&self.message_svc);
        let user_svc = Arc::clone(&self.user_svc);

        let platform_str = "twitch-eventsub".to_string();
        let plat = platform_str.clone();
        let user_id_str = credential.user_id.to_string();
        let user_id_str_clone = user_id_str.clone();

        let join_handle = tokio::spawn(async move {
            // Create a new instance of our EventSub stub.
            let mut eventsub = TwitchEventSubPlatform::new();
            eventsub.credentials = Some(credential.clone());

            // Attempt to connect.
            if let Err(err) = eventsub.connect().await {
                error!("[TwitchEventSub] connect error: {:?}", err);
                return;
            }
            info!("[TwitchEventSub] Connected for user_id={}", user_id_str_clone);

            // Main loop: poll for incoming EventSub events (stubbed for now)
            while let Some(evt) = eventsub.next_message_event().await {
                // In a full implementation you might forward this event
                // to your message service or event bus.
                info!("[TwitchEventSub] Received event: {:?}", evt);
            }
            info!("[TwitchEventSub] Task ended for user_id={}", user_id_str_clone);
        });

        Ok(PlatformRuntimeHandle {
            join_handle,
            platform: plat,
            user_id: user_id_str,
            twitch_irc_instance: None,
        })
    }

    // -------------------------------------------------------------
    // Implementation for the TTV subcommands against Twitch IRC
    // -------------------------------------------------------------

    /// Joins a Twitch IRC channel under the given account_name (global_username).
    pub async fn join_twitch_irc_channel(&self, account_name: &str, channel: &str) -> Result<(), Error> {
        // 1) find user by global_username
        let user = self.user_svc.find_user_by_global_username(account_name).await?;

        // 2) find the active runtime for (platform="twitch-irc", user_id)
        let key = ("twitch-irc".to_string(), user.user_id.to_string());
        let guard = self.active_runtimes.lock().await;
        let handle_opt = guard.get(&key);

        if let Some(handle) = handle_opt {
            // 3) If we have an actual `TwitchIrcPlatform` reference, call join_channel
            if let Some(irc_arc) = &handle.twitch_irc_instance {
                let mut irc_lock = irc_arc.lock().await;
                irc_lock.join_channel(channel).await?;
                Ok(())
            } else {
                Err(Error::Platform(format!(
                    "No TwitchIrcPlatform instance found for account='{}'",
                    account_name
                )))
            }
        } else {
            Err(Error::Platform(format!(
                "No active twitch-irc runtime for account='{}'. Did you run 'start twitch-irc <account>'?",
                account_name
            )))
        }
    }

    /// Parts a Twitch IRC channel under the given account_name.
    pub async fn part_twitch_irc_channel(&self, account_name: &str, channel: &str) -> Result<(), Error> {
        // Same pattern
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
                    "No TwitchIrcPlatform instance found for account='{}'",
                    account_name
                )))
            }
        } else {
            Err(Error::Platform(format!(
                "No active twitch-irc runtime for account='{}'. Did you run 'start twitch-irc <account>'?",
                account_name
            )))
        }
    }

    /// Sends a message to the specified IRC channel via the active Twitch IRC connection.
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
                    "No TwitchIrcPlatform instance found for account='{}'",
                    account_name
                )))
            }
        } else {
            Err(Error::Platform(format!(
                "No active twitch-irc runtime for account='{}'. Did you run 'start twitch-irc <account>'?",
                account_name
            )))
        }
    }
}