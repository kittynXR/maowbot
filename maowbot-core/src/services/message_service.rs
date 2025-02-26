use std::sync::Arc;
use tokio::sync::Mutex;
use chrono::{DateTime, Utc};
use tracing::{info, error};
use uuid::Uuid;

use crate::cache::{ChatCache, CachedMessage};
use crate::eventbus::{EventBus, BotEvent};
use crate::Error;
use crate::repositories::postgres::user_analysis::PostgresUserAnalysisRepository;

use crate::auth::user_manager::{UserManager, DefaultUserManager};
use crate::models::Platform;
use crate::services::user_service::UserService;

/// The MessageService is responsible for ingesting new chat messages from any platform,
/// caching them in memory for short-term usage, and publishing them to the event bus for
/// database logging.
pub struct MessageService {
    chat_cache: Arc<Mutex<ChatCache<PostgresUserAnalysisRepository>>>,
    event_bus: Arc<EventBus>,
    user_manager: Arc<DefaultUserManager>,
    user_service: Arc<UserService>,
}

impl MessageService {
    pub fn new(
        chat_cache: Arc<Mutex<ChatCache<PostgresUserAnalysisRepository>>>,
        event_bus: Arc<EventBus>,
        user_manager: Arc<DefaultUserManager>,
        user_service: Arc<UserService>,
    ) -> Self {
        Self {
            chat_cache,
            event_bus,
            user_manager,
            user_service,
        }
    }

    /// Accept a new chat message from a given platform/channel. We store the numeric
    /// platform_user_id, plus an optional `display_name` if provided, and the user roles.
    ///
    /// This method:
    /// 1) Ensures there's a DB user row with `platform_user_id` in `platform_identities`.
    ///    - If new, sets the `platform_username = display_name` (if given) and also sets
    ///      the user’s `global_username = display_name` if user didn’t exist yet.
    /// 2) Updates roles.
    /// 3) Puts the message in the in-memory `ChatCache`.
    /// 4) Publishes an event with the final DB user_id for logging.
    pub async fn process_incoming_message(
        &self,
        platform: &str,
        channel: &str,
        platform_user_id: &str,
        maybe_display_name: Option<&str>,
        roles_list: &[String],
        text: &str,
    ) -> Result<(), Error> {
        // 1) Convert to a known `Platform`
        let platform_enum = match platform {
            "twitch-irc" => Platform::TwitchIRC,
            "twitch" => Platform::Twitch,
            "discord" => Platform::Discord,
            "vrchat" => Platform::VRChat,
            "twitch-eventsub" => Platform::TwitchEventSub,
            other => return Err(Error::Platform(format!("Unknown platform: {}", other))),
        };

        // 2) Create or retrieve the user in the DB. The numeric ID is stored in `platform_user_id`.
        //    If the user is brand-new, we also use `maybe_display_name` to set global_username.
        let user = self.user_manager
            .get_or_create_user(platform_enum.clone(), platform_user_id, maybe_display_name)
            .await?;

        // 3) Update roles
        if !roles_list.is_empty() {
            if let Err(e) = self.user_service
                .unify_platform_roles(user.user_id, platform_enum.clone(), roles_list)
                .await
            {
                error!("Failed to unify roles in DB: {:?}", e);
            }
        }

        // 4) Insert into our ChatCache
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
            let mut lock = self.chat_cache.lock().await;
            lock.add_message(cached_msg).await;
        }

        // 5) Publish the event with the real DB user’s UUID
        let event = BotEvent::ChatMessage {
            platform: platform.to_string(),
            channel: channel.to_string(),
            user: user.user_id.to_string(), // the correct DB user_id
            text: text.to_string(),
            timestamp: Utc::now(),
        };
        self.event_bus.publish(event).await;

        Ok(())
    }

    /// Get recent messages from the ChatCache
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
