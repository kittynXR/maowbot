//! src/eventbus/db_logger.rs
//!
//! Demonstrates how to spawn a task that subscribes to the EventBus and performs
//! batch inserts into your analytics or chat_messages table.
//!
//! This example logs `BotEvent::ChatMessage` into a hypothetical DB method.

use super::{EventBus, BotEvent};
use std::time::Duration;
use tokio::sync::mpsc::Receiver;
use tokio::time::{sleep, Instant};
use crate::repositories::sqlite::analytics::{AnalyticsRepo, SqliteAnalyticsRepository}; // Example
use crate::repositories::sqlite::analytics::ChatMessage;                // Example
use chrono::{NaiveDateTime, Utc};
use tracing::{info, error};
use crate::Error;

/// Spawns an asynchronous task to receive events from the bus
/// and batch-write them to the database.
pub fn spawn_db_logger_task<T>(
    event_bus: &EventBus,
    analytics_repo: T,
    buffer_size: usize,
    flush_interval_sec: u64,
)
where
    T: AnalyticsRepo + 'static,
{
    let mut rx = event_bus.subscribe(Some(buffer_size));
    let mut shutdown_rx = event_bus.shutdown_rx.clone();

    tokio::spawn(async move {
        let mut buffer = Vec::with_capacity(buffer_size);
        let mut last_flush = Instant::now();
        let flush_interval = Duration::from_secs(flush_interval_sec);

        info!("DB logger task started with batch_size={} flush_interval={}s",
              buffer_size, flush_interval_sec);

        // MAIN LOOP
        loop {
            tokio::select! {
                maybe_event = rx.recv() => {
                    match maybe_event {
                        Some(event) => {
                            // Add to buffer
                            if let Some(cm) = convert_to_chat_message(&event) {
                                buffer.push(cm);
                            }
                            // If buffer is full, flush now
                            if buffer.len() >= buffer_size {
                                if let Err(e) = insert_batch(&analytics_repo, &mut buffer).await {
                                    error!("Error inserting batch: {:?}", e);
                                }
                            }
                        },
                        None => {
                            // Channel closed => break, but we will flush after the loop
                            info!("DB logger channel closed => break from loop.");
                            break;
                        }
                    }
                },
                Ok(_) = shutdown_rx.changed() => {
                    if *shutdown_rx.borrow() {
                        // We received a shutdown => break from loop
                        info!("DB logger shutting down => break from loop.");
                        break;
                    }
                },
                _ = sleep(flush_interval) => {
                    // Periodic flush
                    if !buffer.is_empty() && last_flush.elapsed() >= flush_interval {
                        if let Err(e) = insert_batch(&analytics_repo, &mut buffer).await {
                            error!("Periodic flush error: {:?}", e);
                        }
                        last_flush = Instant::now();
                    }
                }
            }
        }

        // LOOP HAS ENDED => Do a final flush if any messages remain
        if !buffer.is_empty() {
            info!("DB logger final flush after loop exit. {} messages remain.", buffer.len());
            if let Err(e) = insert_batch(&analytics_repo, &mut buffer).await {
                error!("Final flush error: {:?}", e);
            }
        }
        info!("DB logger task exited completely.");
    });
}

/// Attempts to convert a BotEvent into a ChatMessage for logging.
fn convert_to_chat_message(event: &BotEvent) -> Option<ChatMessage> {
    if let BotEvent::ChatMessage { platform, channel, user, text, timestamp } = event {
        Some(ChatMessage {
            message_id: uuid::Uuid::new_v4().to_string(),
            platform: platform.clone(),
            channel: channel.clone(),
            user_id: user.clone(),
            message_text: text.clone(),
            timestamp: timestamp.naive_utc(),
            metadata: None,
        })
    } else {
        None
    }
}

/// Insert a batch of ChatMessages into the DB, then clear the buffer.
async fn insert_batch<T: AnalyticsRepo>(
    repo: &T,
    buffer: &mut Vec<ChatMessage>,
) -> Result<(), Error> {
    if buffer.is_empty() {
        return Ok(());
    }
    for msg in buffer.iter() {
        repo.insert_chat_message(msg).await?;
    }
    buffer.clear();
    Ok(())
}
