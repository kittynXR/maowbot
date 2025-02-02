// File: src/services/message_service.rs

use std::sync::Arc;
use tokio::sync::Mutex;

use chrono::Utc;
use tracing::{info, error};

use crate::cache::ChatCache;
use crate::cache::CachedMessage;
use crate::eventbus::{EventBus, BotEvent};
use crate::Error;

/// `MessageService` handles new messages:
///  1) Insert into in-memory ChatCache
///  2) Publish a `BotEvent::ChatMessage` to the EventBus, so the DB logger can batch it
///
/// We can also do any on-the-fly filtering or spam logic here.
pub struct MessageService {
    chat_cache: Arc<Mutex<ChatCache<crate::repositories::postgres::user_analysis::PostgresUserAnalysisRepository>>>,
    event_bus: Arc<EventBus>,
}

impl MessageService {
    pub fn new(
        chat_cache: Arc<Mutex<ChatCache<crate::repositories::postgres::user_analysis::PostgresUserAnalysisRepository>>>,
        event_bus: Arc<EventBus>,
    ) -> Self {
        Self {
            chat_cache,
            event_bus,
        }
    }

    /// Called when a new message arrives from any platform.
    /// We store it in the ChatCache, then publish an event for DB logging.
    pub async fn process_incoming_message(
        &self,
        platform: &str,
        channel: &str,
        user_id: &str,
        text: &str,
    ) -> Result<(), Error> {

        // 1) Estimate or compute token count. For demonstration, we’ll do something naive:
        let token_count = text.split_whitespace().count();

        // 2) Build a `CachedMessage`
        let msg = CachedMessage {
            platform: platform.to_string(),
            channel: channel.to_string(),
            user_id: user_id.to_string(),
            text: text.to_string(),
            timestamp: Utc::now().naive_utc(),
            token_count,
        };

        // 3) Insert into in-memory cache
        {
            let mut cache_lock = self.chat_cache.lock().await;
            cache_lock.add_message(msg.clone()).await;
        }

        // 4) Publish to event bus so DB logger will store it
        let event = BotEvent::ChatMessage {
            platform: platform.to_string(),
            channel: channel.to_string(),
            user: user_id.to_string(), // user_id is not the platform “username,” it’s our internal user ID
            text: text.to_string(),
            timestamp: Utc::now(),
        };
        self.event_bus.publish(event).await;

        Ok(())
    }

    /// We can add more methods for retrieving recent messages from the cache, e.g. for LLM prompt building
    pub async fn get_recent_messages(
        &self,
        since: chrono::NaiveDateTime,
        token_limit: Option<usize>,
        filter_user_id: Option<&str>,
    ) -> Vec<CachedMessage> {
        let cache_lock = self.chat_cache.lock().await;
        cache_lock.get_recent_messages(since, token_limit, filter_user_id)
    }
}
