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

    /// `user_name_with_roles` might look like `"kittyncat|roles=mod,vip,subscriber"`
    /// if it comes from Twitch or Discord. We parse any roles after "|roles=" and
    /// store them in the DB as soon as we see the user.
    pub async fn process_incoming_message(
        &self,
        platform: &str,
        channel: &str,
        user_name_with_roles: &str,
        text: &str,
    ) -> Result<(), Error> {

        // 1) Parse out the roles, if any:
        let (raw_name, roles_list) = if let Some(idx) = user_name_with_roles.find("|roles=") {
            let nm = &user_name_with_roles[..idx];
            let roles_str = &user_name_with_roles[idx + 7..]; // skip "|roles="
            (nm.trim().to_string(), roles_str.split(',').map(|s| s.trim().to_string()).collect::<Vec<_>>())
        } else {
            (user_name_with_roles.to_string(), vec![])
        };
        tracing::info!(?raw_name, ?roles_list, "parsed user & roles");

        // 2) Convert to a known `Platform`
        let platform_enum = match platform {
            "twitch-irc" => Platform::TwitchIRC,
            "twitch" => Platform::Twitch,
            "discord" => Platform::Discord,
            "vrchat" => Platform::VRChat,
            "twitch-eventsub" => Platform::TwitchEventSub,
            other => return Err(Error::Platform(format!("Unknown platform: {}", other))),
        };

        // 3) Get/create the user in DB. We'll pass `raw_name` as the platform_user_id
        //    and also store roles in platform_identities if present.
        let user = self.user_manager
            .get_or_create_user(platform_enum.clone(), &raw_name, Some(&raw_name))
            .await?;

        // => unify the roles with the stored platform_identity
        if !roles_list.is_empty() {
            if let Err(e) = self.user_service.unify_platform_roles(user.user_id, platform_enum.clone(), &roles_list).await {
                error!("Failed to unify roles in DB: {:?}", e);
            }
        }

        // 4) Insert the message into our in-memory ChatCache
        let token_count = text.split_whitespace().count();
        let cached_msg = CachedMessage {
            platform: platform.to_string(),
            channel: channel.to_string(),
            user_name: raw_name.clone(),
            text: text.to_string(),
            timestamp: Utc::now(),
            token_count,
            user_roles: roles_list.clone(),
        };
        {
            let mut lock = self.chat_cache.lock().await;
            lock.add_message(cached_msg).await;
        }

        // 5) Publish an event with the **user_id** so DB logger can store chat_messages.user_id
        let event = BotEvent::ChatMessage {
            platform: platform.to_string(),
            channel: channel.to_string(),
            user: user.user_id.to_string(),
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