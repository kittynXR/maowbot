// File: maowbot-core/src/eventbus/db_logger.rs

use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::{sleep, Instant};
use tokio::task::JoinHandle;
use tracing::{info, error};

use chrono::Utc;
use crate::Error;
use crate::eventbus::{EventBus, BotEvent};
use crate::repositories::postgres::analytics::{AnalyticsRepo, ChatMessage};

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
    let event_bus_cloned = event_bus.clone();
    let mut shutdown_rx = event_bus.shutdown_rx.clone();

    tokio::spawn(async move {
        let mut rx = event_bus_cloned.subscribe(Some(buffer_size)).await;

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
    })
}

fn convert_to_chat_message(event: &BotEvent) -> Option<ChatMessage> {
    if let BotEvent::ChatMessage { platform, channel, user, text, timestamp } = event {
        Some(ChatMessage {
            message_id: uuid::Uuid::new_v4(),
            platform: platform.clone(),
            channel: channel.clone(),
            user_id: user.parse().unwrap_or_else(|_| uuid::Uuid::nil()),
            message_text: text.clone(),
            timestamp: timestamp.to_utc(),
            metadata: None, // We can fill in metadata if we want
        })
    } else {
        None
    }
}

/// Bulk-insert the entire buffer at once.
async fn insert_batch<T: AnalyticsRepo>(
    repo: &T,
    buffer: &mut Vec<ChatMessage>,
) -> Result<(), Error> {
    if buffer.is_empty() {
        return Ok(());
    }
    // Use the new bulk method
    repo.insert_chat_messages(buffer).await?;
    buffer.clear();
    Ok(())
}