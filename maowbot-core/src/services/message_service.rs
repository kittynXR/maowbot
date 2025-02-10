// File: src/services/message_service.rs

use std::sync::Arc;
use tokio::sync::Mutex;
use chrono::{DateTime, Utc};
use tracing::{info, error};
use uuid::Uuid;
use crate::cache::{ChatCache, CachedMessage};
use crate::eventbus::{EventBus, BotEvent};
use crate::Error;
use crate::repositories::postgres::user_analysis::PostgresUserAnalysisRepository;

pub struct MessageService {
    chat_cache: Arc<Mutex<ChatCache<PostgresUserAnalysisRepository>>>,
    event_bus: Arc<EventBus>,
}

impl MessageService {
    pub fn new(
        chat_cache: Arc<Mutex<ChatCache<PostgresUserAnalysisRepository>>>,
        event_bus: Arc<EventBus>,
    ) -> Self {
        Self {
            chat_cache,
            event_bus,
        }
    }

    pub async fn process_incoming_message(
        &self,
        platform: &str,
        channel: &str,
        user_id: &Uuid,
        text: &str,
    ) -> Result<(), Error> {

        let token_count = text.split_whitespace().count();

        let msg = CachedMessage {
            platform: platform.to_string(),
            channel: channel.to_string(),
            user_id: user_id.to_string(),
            text: text.to_string(),
            timestamp: Utc::now(),
            token_count,
        };

        {
            let mut cache_lock = self.chat_cache.lock().await;
            cache_lock.add_message(msg.clone()).await;
        }

        let event = BotEvent::ChatMessage {
            platform: platform.to_string(),
            channel: channel.to_string(),
            user: user_id.to_string(),
            text: text.to_string(),
            timestamp: Utc::now(),
        };
        self.event_bus.publish(event).await;

        Ok(())
    }

    pub async fn get_recent_messages(
        &self,
        since: DateTime<Utc>,
        token_limit: Option<usize>,
        filter_user_id: Option<&str>,
    ) -> Vec<CachedMessage> {
        let cache_lock = self.chat_cache.lock().await;
        cache_lock.get_recent_messages(since, token_limit, filter_user_id).await
    }
}