use std::sync::Arc;
use tokio::sync::Mutex;
use chrono::{DateTime, Utc};
use tracing::{debug, info, error, warn};
use maowbot_common::models::cache::CachedMessage;
use maowbot_common::models::platform::Platform;
use maowbot_common::traits::repository_traits::CredentialsRepository;
use crate::eventbus::{EventBus, BotEvent};
use crate::Error;
use crate::repositories::postgres::user_analysis::PostgresUserAnalysisRepository;

use crate::auth::user_manager::{UserManager, DefaultUserManager};
use crate::cache::message_cache::ChatCache;
use crate::services::user_service::UserService;
use crate::services::{CommandService, CommandResponse};
use crate::platforms::manager::PlatformManager;

/// The MessageService is responsible for ingesting new chat messages from any platform
/// and for checking/processing commands (via CommandService).
pub struct MessageService {
    chat_cache: Arc<Mutex<ChatCache<PostgresUserAnalysisRepository>>>,
    event_bus: Arc<EventBus>,
    user_manager: Arc<DefaultUserManager>,
    pub user_service: Arc<UserService>,
    command_service: Arc<CommandService>,
    platform_manager: Arc<PlatformManager>,
    credentials_repo: Arc<dyn CredentialsRepository + Send + Sync>,
}

impl MessageService {
    pub fn new(
        chat_cache: Arc<Mutex<ChatCache<PostgresUserAnalysisRepository>>>,
        event_bus: Arc<EventBus>,
        user_manager: Arc<DefaultUserManager>,
        user_service: Arc<UserService>,
        command_service: Arc<CommandService>,
        platform_manager: Arc<PlatformManager>,
        credentials_repo: Arc<dyn CredentialsRepository + Send + Sync>,
    ) -> Self {
        debug!("MessageService::new() called");
        Self {
            chat_cache,
            event_bus,
            user_manager,
            user_service,
            command_service,
            platform_manager,
            credentials_repo,
        }
    }

    /// Processes an incoming chat message:
    ///  1. Converts platform string to enum.
    ///  2. Retrieves (or creates) the user.
    ///  3. Updates user roles if provided.
    ///  4. Stores the message in the cache.
    ///  5. Publishes the chat event to the EventBus.
    ///  6. Checks for a command response from CommandService; if found, sends the lines.
    pub async fn process_incoming_message(
        &self,
        platform: &str,
        channel: &str,
        platform_user_id: &str,
        maybe_display_name: Option<&str>,
        roles_list: &[String],
        text: &str,
    ) -> Result<(), Error> {
        debug!("process_incoming_message() called for platform='{}', channel='{}'", platform, channel);

        // 1) Convert platform to enum
        let platform_enum = match platform {
            // "twitch-irc" | "twitch" => Platform::TwitchIRC,
            "twitch-irc" | "twitch" => Platform::Twitch,
            "discord" => Platform::Discord,
            "vrchat" => Platform::VRChat,
            "twitch-eventsub" => Platform::TwitchEventSub,
            other => {
                error!("Unknown platform: {}", other);
                return Err(Error::Platform(format!("Unknown platform: {}", other)));
            }
        };

        // 2) Get or create the user
        let user = self.user_manager
            .get_or_create_user(platform_enum.clone(), platform_user_id, maybe_display_name)
            .await?;

        // 3) Update roles if provided
        if !roles_list.is_empty() {
            if let Err(e) = self.user_service
                .unify_platform_roles(user.user_id, platform_enum.clone(), roles_list)
                .await
            {
                error!("Failed to unify roles for user {:?}: {:?}", user.user_id, e);
            }
        }

        // 4) Add message to chat cache
        let token_count = text.split_whitespace().count();
        let cached_msg = CachedMessage {
            platform: platform.to_string(),
            channel: channel.to_string(),
            user_name: user.global_username.clone().unwrap_or_else(|| platform_user_id.to_string()),
            text: text.to_string(),
            timestamp: Utc::now(),
            token_count,
            user_roles: roles_list.to_vec(),
        };
        {
            let lock = self.chat_cache.lock().await;
            lock.add_message(cached_msg).await;
        }

        // 5) Publish chat event
        let event = BotEvent::ChatMessage {
            platform: platform.to_string(),
            channel: channel.to_string(),
            user: user.user_id.to_string(),
            text: text.to_string(),
            timestamp: Utc::now(),
        };
        self.event_bus.publish(event).await;

        // 6) Check if it's a command
        let is_stream_online = false; // (placeholder: integrate with actual stream status if needed)
        match self.command_service
            .handle_chat_line(
                platform,
                channel,
                user.user_id,
                roles_list,
                text,
                is_stream_online,
            )
            .await?
        {
            Some(CommandResponse {
                     texts,
                     respond_credential_id,
                     platform: cmd_platform,
                     channel: cmd_channel,
                 }) => {
                // figure out which account to send from, if it's Twitch-IRC
                if cmd_platform.eq_ignore_ascii_case("twitch-irc") {
                    let account_name = if let Some(cid) = respond_credential_id {
                        // If the command had a specific credential attached:
                        if let Some(cred) = self.credentials_repo.get_credential_by_id(cid).await? {
                            cred.user_name
                        } else {
                            warn!("Respond credential_id not found in DB. Using secondary account if set.");
                            self.get_ttv_secondary_or_warn().await
                        }
                    } else {
                        // No respond_credential => use the “secondary” account from bot_config
                        self.get_ttv_secondary_or_warn().await
                    };

                    for line in texts {
                        if let Err(e) = self.platform_manager
                            .send_twitch_irc_message(&account_name, &cmd_channel, &line)
                            .await
                        {
                            warn!("Failed to send IRC reply: {:?}", e);
                        }
                    }
                } else if cmd_platform.eq_ignore_ascii_case("discord") {
                    // Potentially implement a Discord message-sending function, if desired
                    info!("(Discord) would send multi-line: {:?}", texts);
                } else {
                    info!("(Other) command response => platform='{}', lines={:?}", cmd_platform, texts);
                }
            }
            None => {
                // no command response
            }
        }

        Ok(())
    }

    /// Returns the secondary Twitch-IRC account name from bot_config, or logs a warning if unset.
    /// Returns a placeholder string if not configured.
    async fn get_ttv_secondary_or_warn(&self) -> String {
        let val = self.command_service
            .bot_config_repo
            .get_value("ttv_secondary_account")
            .await;

        match val {
            Ok(Some(acc)) if !acc.trim().is_empty() => acc,
            _ => {
                warn!(
                    "No secondary Twitch-IRC account configured in 'ttv_secondary_account'. \
                     Please set one with 'ttv secondary <account>'."
                );
                "NoSecondaryAccount".to_string()
            }
        }
    }

    /// Returns recent messages from the chat cache.
    pub async fn get_recent_messages(
        &self,
        since: DateTime<Utc>,
        token_limit: Option<usize>,
        filter_user_name: Option<&str>,
    ) -> Vec<CachedMessage> {
        let lock = self.chat_cache.lock().await;
        lock.get_recent_messages(since, token_limit, filter_user_name).await
    }
}
