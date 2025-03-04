// File: maowbot-core/src/services/message_service.rs

use std::sync::Arc;
use tokio::sync::Mutex;
use chrono::{DateTime, Utc};
use tracing::{debug, info, error, warn};
use uuid::Uuid;

use crate::cache::{ChatCache, CachedMessage};
use crate::eventbus::{EventBus, BotEvent};
use crate::Error;
use crate::repositories::postgres::user_analysis::PostgresUserAnalysisRepository;

use crate::auth::user_manager::{UserManager, DefaultUserManager};
use crate::models::Platform;
use crate::services::user_service::UserService;
use crate::services::command_service::{CommandService, CommandResponse};
use crate::platforms::manager::PlatformManager;
use crate::repositories::postgres::credentials::CredentialsRepository;

/// The MessageService is responsible for ingesting new chat messages from any platform
/// and for checking/processing commands.
pub struct MessageService {
    chat_cache: Arc<Mutex<ChatCache<PostgresUserAnalysisRepository>>>,
    event_bus: Arc<EventBus>,
    user_manager: Arc<DefaultUserManager>,
    user_service: Arc<UserService>,

    /// Used to handle chat commands.
    command_service: Arc<CommandService>,
    /// Used to send replies back to chat.
    platform_manager: Arc<PlatformManager>,

    /// For credential lookups.
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
    ///  3. Updates user roles.
    ///  4. Stores the message in the cache.
    ///  5. Publishes the chat event.
    ///  6. Checks if the message is a command; if so, sends the reply.
    pub async fn process_incoming_message(
        &self,
        platform: &str,
        channel: &str,
        platform_user_id: &str,
        maybe_display_name: Option<&str>,
        roles_list: &[String],
        text: &str,
    ) -> Result<(), Error> {
        debug!("process_incoming_message() called for platform: '{}', channel: '{}'", platform, channel);

        // 1) Convert platform to enum
        let platform_enum = match platform {
            "twitch-irc" => Platform::TwitchIRC,
            "twitch" => Platform::Twitch,
            "discord" => Platform::Discord,
            "vrchat" => Platform::VRChat,
            "twitch-eventsub" => Platform::TwitchEventSub,
            other => {
                error!("Unknown platform: {}", other);
                return Err(Error::Platform(format!("Unknown platform: {}", other)));
            }
        };
        debug!("Converted platform '{}' to enum {:?}", platform, platform_enum);

        // 2) Get or create the user
        let user = self.user_manager
            .get_or_create_user(platform_enum.clone(), platform_user_id, maybe_display_name)
            .await?;
        debug!("User retrieved/created: {:?}", user);

        // 3) Update roles if provided
        if !roles_list.is_empty() {
            debug!("Updating roles for user {:?}: {:?}", user.user_id, roles_list);
            if let Err(e) = self.user_service
                .unify_platform_roles(user.user_id, platform_enum.clone(), roles_list)
                .await
            {
                error!("Failed to update roles for user {:?}: {:?}", user.user_id, e);
            } else {
                debug!("Roles updated successfully for user {:?}", user.user_id);
            }
        } else {
            debug!("No roles provided for user {:?}", user.user_id);
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
        debug!("Storing message in ChatCache: {:?}", cached_msg);
        {
            let mut lock = self.chat_cache.lock().await;
            lock.add_message(cached_msg).await;
        }
        debug!("Message stored in ChatCache");

        // 5) Publish chat event
        let event = BotEvent::ChatMessage {
            platform: platform.to_string(),
            channel: channel.to_string(),
            user: user.user_id.to_string(),
            text: text.to_string(),
            timestamp: Utc::now(),
        };
        debug!("Publishing BotEvent: {:?}", event);
        self.event_bus.publish(event).await;
        debug!("BotEvent published");

        // 6) Check for command response
        debug!("Checking if message is a command...");
        match self.command_service
            .handle_chat_line(
                platform,
                channel,
                user.user_id,
                roles_list,
                text,
                false, // is_stream_online (set as needed)
            )
            .await?
        {
            Some(CommandResponse {
                     text: reply_text,
                     respond_credential_id,
                     platform: cmd_platform,
                     channel: cmd_channel,
                 }) => {
                debug!("Command detected. Reply text: '{}'", reply_text);
                if cmd_platform.eq_ignore_ascii_case("twitch-irc") {
                    let account_name = if let Some(cid) = respond_credential_id {
                        debug!("Looking up credential for id: {:?}", cid);
                        if let Some(cred) = self.credentials_repo.get_credential_by_id(cid).await? {
                            debug!("Credential found: {:?}", cred);
                            cred.user_name
                        } else {
                            warn!("No credential found for id: {:?}", cid);
                            "DefaultIrcAccount".to_string()
                        }
                    } else {
                        debug!("No respond_with_credential set; using default account");
                        "DefaultIrcAccount".to_string()
                    };

                    debug!("Sending IRC reply from account '{}' to channel '{}': {}",
                           account_name, cmd_channel, reply_text);
                    if let Err(e) = self.platform_manager
                        .send_twitch_irc_message(&account_name, &cmd_channel, &reply_text)
                        .await
                    {
                        warn!("Failed to send IRC reply: {:?}", e);
                    } else {
                        debug!("IRC reply sent successfully");
                    }
                } else if cmd_platform.eq_ignore_ascii_case("discord") {
                    info!("Would send Discord reply to channel '{}': {}", cmd_channel, reply_text);
                } else {
                    info!("Command response for platform '{}' not implemented. Reply: '{}'",
                          cmd_platform, reply_text);
                }
            }
            None => {
                debug!("Message is not a command or no response is required.");
            }
        }

        Ok(())
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
