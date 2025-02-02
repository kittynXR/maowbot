// maowbot-core/src/tasks/cache_maintenance.rs

use std::time::Duration;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tokio::time::sleep;
use crate::cache::ChatCache;
use crate::repositories::postgres::user_analysis::PostgresUserAnalysisRepository;

/// Spawns a background task that periodically prunes old messages from the ChatCache.
pub fn spawn_cache_prune_task(
    cache: Arc<Mutex<ChatCache<PostgresUserAnalysisRepository>>>,
    interval: Duration,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            sleep(interval).await;
            let mut locked_cache = cache.lock().await;
            locked_cache.trim_old_messages().await;
        }
    })
}