//! src/eventbus/db_logger.rs
//!
//! Spawns a task that subscribes to the EventBus, buffers BotEvent::ChatMessage,
//! and flushes them to the DB. Drains the queue on shutdown, then does a final flush.

use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::{sleep, Instant};
use tokio::task::JoinHandle;
use tracing::{info, error};

use chrono::Utc;
use crate::Error;
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
    // Now await the subscription so that rx is a Receiver<BotEvent>
    let mut rx = futures_lite::future::block_on(event_bus.subscribe(Some(buffer_size)));
    let mut shutdown_rx = event_bus.shutdown_rx.clone();

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
                            if let Some(cm) = convert_to_chat_message(&event) {
                                buffer.push(cm);
                            }
                            if buffer.len() >= buffer_size {
                                if let Err(e) = insert_batch(&analytics_repo, &mut buffer).await {
                                    error!("Error inserting batch: {:?}", e);
                                }
                                last_flush = Instant::now();
                            }
                        },
                        None => {
                            info!("DB logger channel closed => break from loop.");
                            break;
                        }
                    }
                },
                Ok(_) = shutdown_rx.changed() => {
                    if *shutdown_rx.borrow() {
                        info!("DB logger shutting down => break from loop.");
                        break;
                    }
                },
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

        info!("DB logger: draining any remaining messages after loop exit.");
        while let Ok(event) = rx.try_recv() {
            if let Some(cm) = convert_to_chat_message(&event) {
                buffer.push(cm);
            }
        }

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

fn convert_to_chat_message(event: &BotEvent) -> Option<ChatMessage> {
    if let BotEvent::ChatMessage { platform, channel, user, text, timestamp } = event {
        Some(ChatMessage {
            message_id: uuid::Uuid::new_v4().to_string(),
            platform: platform.clone(),
            channel: channel.clone(),
            user_id: user.clone(),
            message_text: text.clone(),
            timestamp: timestamp.timestamp(),
            metadata: None,
        })
    } else {
        None
    }
}

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