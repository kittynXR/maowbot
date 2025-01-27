//! src/eventbus/db_logger.rs
//!
//! Spawns a task that subscribes to the EventBus, buffers BotEvent::ChatMessage,
//! and flushes them to the DB. Drains the queue on shutdown, then does a final flush.

use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::{sleep, Instant};
use tokio::task::JoinHandle;
use tracing::{info, error};

use chrono::{Utc};
use crate::{Error};
use crate::eventbus::{EventBus, BotEvent};
use crate::repositories::sqlite::analytics::{AnalyticsRepo, ChatMessage};


/// Spawns an asynchronous task to receive events from the bus
/// and batch-write them to the database. Returns a `JoinHandle<()>`
/// so the caller can `.await` the final flush in tests or shutdown logic.
pub fn spawn_db_logger_task<T>(
    event_bus: &EventBus,
    analytics_repo: T,
    buffer_size: usize,
    flush_interval_sec: u64,
) -> JoinHandle<()>
where
    T: AnalyticsRepo + 'static,
{
    // Subscribe with a bounded channel
    let mut rx = event_bus.subscribe(Some(buffer_size));

    // Watch channel for shutdown
    let mut shutdown_rx = event_bus.shutdown_rx.clone();

    // Spawn the logger in a dedicated task
    let handle = tokio::spawn(async move {
        let mut buffer = Vec::with_capacity(buffer_size);
        let flush_interval = Duration::from_secs(flush_interval_sec);
        let mut last_flush = Instant::now();

        info!(
            "DB logger task started with batch_size={} flush_interval={}s",
            buffer_size, flush_interval_sec
        );

        // MAIN LOOP
        loop {
            tokio::select! {
                biased;
                maybe_event = rx.recv() => {
                    match maybe_event {
                        Some(event) => {
                            // Convert and buffer
                            if let Some(cm) = convert_to_chat_message(&event) {
                                buffer.push(cm);
                            }
                            // If buffer is full, flush now
                            if buffer.len() >= buffer_size {
                                if let Err(e) = insert_batch(&analytics_repo, &mut buffer).await {
                                    error!("Error inserting batch: {:?}", e);
                                }
                                last_flush = Instant::now();
                            }
                        },
                        None => {
                            // The sending side closed => break
                            info!("DB logger channel closed => break from loop.");
                            break;
                        }
                    }
                },

                // If the EventBus sets shutdown to true, break out
                Ok(_) = shutdown_rx.changed() => {
                    if *shutdown_rx.borrow() {
                        info!("DB logger shutting down => break from loop.");
                        break;
                    }
                },

                // Periodic flush if buffer is non-empty
                _ = sleep(flush_interval) => {
                    if !buffer.is_empty() && last_flush.elapsed() >= flush_interval {
                        if let Err(e) = insert_batch(&analytics_repo, &mut buffer).await {
                            error!("Periodic flush error: {:?}", e);
                        }
                        last_flush = Instant::now();
                    }
                }
            }
        }

        // OUTSIDE THE LOOP => final drain of leftover items in `rx`
        info!("DB logger: draining any remaining messages after loop exit.");
        while let Ok(event) = rx.try_recv() {
            if let Some(cm) = convert_to_chat_message(&event) {
                buffer.push(cm);
            }
        }

        // Final flush
        if !buffer.is_empty() {
            info!("DB logger final flush: {} messages remain.", buffer.len());
            if let Err(e) = insert_batch(&analytics_repo, &mut buffer).await {
                error!("Final flush error: {:?}", e);
            }
        }

        info!("DB logger task exited completely.");
    });

    handle
}

/// Converts a `BotEvent::ChatMessage` into a `ChatMessage` struct for the DB.
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

/// Insert a batch of `ChatMessage`s into the DB, then clear the buffer.
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
