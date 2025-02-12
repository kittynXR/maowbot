use std::collections::HashMap;
use std::sync::Arc;
use tokio::task::JoinHandle;
use tracing::{info, error, warn};

use crate::eventbus::EventBus;
use crate::services::message_service::MessageService;
use crate::services::user_service::UserService;
use crate::Error;
use crate::models::{PlatformCredential, Platform};
use crate::repositories::postgres::credentials::CredentialsRepository;

// Runtimes
use crate::platforms::discord::runtime::DiscordPlatform;
use crate::platforms::PlatformIntegration;
use crate::platforms::twitch_helix::runtime::TwitchPlatform;
use crate::platforms::twitch_irc::runtime::TwitchIrcPlatform;
use crate::platforms::vrchat::runtime::VRChatPlatform;

/// Each active runtime might store info about the platform handle, plus the task join handle.
pub struct PlatformRuntimeHandle {
    pub join_handle: JoinHandle<()>,
    pub platform: String,
    pub user_id: String,
}

/// Manages multiple concurrent platform runtimes.
/// Key = (platform, user_id).
pub struct PlatformManager {
    message_svc: Arc<MessageService>,
    user_svc: Arc<UserService>,
    event_bus: Arc<EventBus>,
    credentials_repo: Arc<dyn CredentialsRepository + Send + Sync>,

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

        // We'll unify the 'platform_str' so it matches the user’s original parse: "discord"
        // Then pass that to process_incoming_message(...) as well.
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
                let user_platform_id = msg_event.user_id;
                let text = msg_event.text;
                let username = msg_event.username;

                // 1) get/create user
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

                // 2) pass to message_svc
                if let Err(e) = message_svc
                    .process_incoming_message(&platform_str.clone(), &channel, &user.user_id, &text)
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
        };
        Ok(rh)
    }

    async fn spawn_twitch_helix(
        &self,
        credential: PlatformCredential
    ) -> Result<PlatformRuntimeHandle, Error> {
        let message_svc = Arc::clone(&self.message_svc);
        let user_svc = Arc::clone(&self.user_svc);

        // Instead of “twitch_helix”, unify to "twitch"
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
                let display_name = msg_event.display_name;

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
                    .process_incoming_message(&platform_str.clone(), &channel, &user.user_id, &text)
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
                let display_name = evt.vrchat_display_name;

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

                if let Err(e) = message_svc
                    .process_incoming_message(&platform_str.clone(), "roomOrWorldId", &user.user_id, &text)
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
        })
    }

    async fn spawn_twitch_irc(
        &self,
        credential: PlatformCredential
    ) -> Result<PlatformRuntimeHandle, Error> {
        let message_svc = Arc::clone(&self.message_svc);
        let user_svc = Arc::clone(&self.user_svc);

        // We'll unify to “twitch-irc” for both the manager handle and the published event.
        let platform_str = "twitch-irc".to_string();
        let plat = platform_str.clone();
        let user_id_str = credential.user_id.to_string();
        let user_id_str_clone = user_id_str.clone();

        let join_handle = tokio::spawn(async move {
            let mut irc = TwitchIrcPlatform {
                credentials: Some(credential.clone()),
                connection_status: crate::platforms::ConnectionStatus::Disconnected,
            };
            if let Err(err) = irc.connect().await {
                error!("[TwitchIRC] connect error: {:?}", err);
                return;
            }
            info!("[TwitchIRC] Connected for user_id={}", user_id_str_clone);

            while let Some(evt) = irc.next_message_event().await {
                let channel = evt.channel;
                let user_platform_id = evt.user_id;
                let text = evt.text;
                let user_name = evt.user_name;

                let user = match user_svc
                    .get_or_create_user("twitch-irc", &user_platform_id, Some(&user_name))
                    .await
                {
                    Ok(u) => u,
                    Err(e) => {
                        error!("[TwitchIRC] user_svc error: {:?}", e);
                        continue;
                    }
                };

                // Now publish the same platform_str = "twitch-irc" here
                if let Err(e) = message_svc
                    .process_incoming_message(&platform_str, &channel, &user.user_id, &text)
                    .await
                {
                    error!("[TwitchIRC] process_incoming_message failed: {:?}", e);
                }
            }

            info!("[TwitchIRC] Task ended for user_id={}", user_id_str_clone);
        });

        Ok(PlatformRuntimeHandle {
            join_handle,
            platform: plat,
            user_id: user_id_str,
        })
    }
}