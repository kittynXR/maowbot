use chrono::{DateTime, Duration, Utc};
use dashmap::DashMap;
use tokio::sync::RwLock;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicUsize, Ordering};
use crate::models::user_analysis::UserAnalysis;
use crate::repositories::postgres::user_analysis::UserAnalysisRepository;

/// Single cached chat message
#[derive(Debug, Clone)]
pub struct CachedMessage {
    pub platform: String,
    pub channel: String,
    /// Now renamed to reflect actual chatter username (not DB UUID).
    pub user_name: String,
    pub text: String,
    pub timestamp: DateTime<Utc>,
    pub token_count: usize,

    /// **NEW**: We store the user's roles (e.g. Twitch "mod", "broadcaster", "subscriber", etc.).
    /// For non-Twitch platforms, this may be empty.
    pub user_roles: Vec<String>,
}

/// Rules for trimming or filtering
#[derive(Debug, Clone)]
pub struct TrimPolicy {
    pub max_age_seconds: Option<i64>,
    pub spam_score_cutoff: Option<f32>,
    pub max_total_messages: Option<usize>,
    pub max_messages_per_user: Option<usize>,
    pub min_quality_score: Option<f32>,
}

/// Config that the ChatCache will use
#[derive(Debug, Clone)]
pub struct CacheConfig {
    pub trim_policy: TrimPolicy,
}

/// Fixed-size ring buffer for chronological messages.
struct GlobalRingBuffer {
    capacity: usize,
    messages: Vec<Option<CachedMessage>>,
    start_idx: usize, // oldest message slot
    end_idx: usize,   // next insertion slot
    total_count: usize, // how many are stored
}

impl GlobalRingBuffer {
    fn new(capacity: usize) -> Self {
        Self {
            capacity,
            messages: vec![None; capacity],
            start_idx: 0,
            end_idx: 0,
            total_count: 0,
        }
    }

    /// Push a new message, overwriting the oldest if full.
    /// Returns the ring index where it was stored.
    fn push(&mut self, msg: CachedMessage) -> usize {
        if self.capacity == 0 {
            return 0;
        }
        if self.total_count == self.capacity {
            // Overwrite oldest
            self.start_idx = (self.start_idx + 1) % self.capacity;
        } else {
            self.total_count += 1;
        }
        let idx = self.end_idx;
        self.messages[idx] = Some(msg);
        self.end_idx = (self.end_idx + 1) % self.capacity;
        idx
    }

    /// Get the message by ring index, if it hasn't been overwritten.
    fn get(&self, ring_idx: usize) -> Option<&CachedMessage> {
        if self.capacity == 0 || self.total_count == 0 {
            return None;
        }
        if !self.is_in_range(ring_idx) {
            return None;
        }
        self.messages[ring_idx].as_ref()
    }

    /// Check if `ring_idx` is in the [start_idx .. start_idx + total_count) window (mod capacity).
    fn is_in_range(&self, ring_idx: usize) -> bool {
        if self.total_count == 0 || self.capacity == 0 {
            return false;
        }
        let end_pos = (self.start_idx + self.total_count) % self.capacity;
        if self.start_idx < end_pos {
            ring_idx >= self.start_idx && ring_idx < end_pos
        } else {
            // wrap
            ring_idx >= self.start_idx || ring_idx < end_pos
        }
    }

    /// Removes messages older than `cutoff` from the front if possible.
    /// Returns the count of removed messages.
    fn trim_by_time(&mut self, cutoff: DateTime<Utc>) -> usize {
        let mut removed = 0;
        while self.total_count > 0 {
            let oldest_idx = self.start_idx;
            if let Some(msg) = &self.messages[oldest_idx] {
                if msg.timestamp < cutoff {
                    self.messages[oldest_idx] = None;
                    self.start_idx = (oldest_idx + 1) % self.capacity;
                    self.total_count -= 1;
                    removed += 1;
                } else {
                    break;
                }
            } else {
                // shouldn't happen if total_count is correct
                self.start_idx = (oldest_idx + 1) % self.capacity;
                self.total_count -= 1;
                removed += 1;
            }
        }
        removed
    }
}

/// In-memory ChatCache with concurrency-friendly data structures.
///
/// - A `tokio::sync::RwLock<GlobalRingBuffer>` for the global ring of messages (fixed capacity).
/// - A `DashMap<user_name, VecDeque<usize>>` storing ring indices for each user.
/// - The `UserAnalysisRepository` for spam checks if you want to do that asynchronously.
pub struct ChatCache<R: UserAnalysisRepository> {
    global: RwLock<GlobalRingBuffer>,
    /// The key is now the `user_name`, so we can limit messages per "username".
    user_map: DashMap<String, VecDeque<usize>>,
    total_in_buffer: AtomicUsize,
    user_analysis_repo: R,
    config: CacheConfig,
}

impl<R: UserAnalysisRepository> ChatCache<R> {
    /// Construct a new ChatCache, using `max_total_messages` as the ring's capacity.
    pub fn new(user_analysis_repo: R, config: CacheConfig) -> Self {
        let capacity = config
            .trim_policy
            .max_total_messages
            .unwrap_or(10_000);

        let ring = GlobalRingBuffer::new(capacity);
        Self {
            global: RwLock::new(ring),
            user_map: DashMap::new(),
            total_in_buffer: AtomicUsize::new(0),
            user_analysis_repo,
            config,
        }
    }

    /// Add a message to the cache, optionally doing a quick synchronous age trim.
    /// (We do not do spam/quality checks here—do that in a separate background job.)
    pub async fn add_message(&self, msg: CachedMessage) {
        // -- 1) RING LOCK (write):
        let idx = {
            let mut guard = self.global.write().await;
            let idx = guard.push(msg.clone());
            self.total_in_buffer.store(guard.total_count, Ordering::Release);
            idx
        }; // ring write lock is dropped here

        // -- 2) DASHMAP lock:
        {
            let mut user_q = self.user_map.entry(msg.user_name.clone()).or_default();
            user_q.push_back(idx);
            if let Some(max_per_user) = self.config.trim_policy.max_messages_per_user {
                while user_q.len() > max_per_user {
                    user_q.pop_front();
                }
            }
        }

        // -- 3) (Optional) RING LOCK (write) to do age trimming:
        if let Some(max_age) = self.config.trim_policy.max_age_seconds {
            let cutoff = Utc::now() - Duration::seconds(max_age);
            let removed = {
                let mut guard = self.global.write().await;
                let removed = guard.trim_by_time(cutoff);
                self.total_in_buffer.store(guard.total_count, Ordering::Release);
                removed
            }; // ring write lock dropped

            if removed > 0 {
                self.remove_stale_indices();
            }
        }
    }

    /// A separate function that you might call from a background task to remove messages from
    /// users who exceed spam score or fail min_quality, etc.
    pub async fn trim_spammy_users(&self) {
        let policy = &self.config.trim_policy;
        if policy.spam_score_cutoff.is_none() && policy.min_quality_score.is_none() {
            return;
        }
        let spam_cut = policy.spam_score_cutoff.unwrap_or(f32::MAX);
        let quality_min = policy.min_quality_score.unwrap_or(-9999.0);

        // 1) Gather user_names to purge
        let mut purge_list = Vec::new();
        for user_name in self.user_map.iter().map(|e| e.key().clone()) {
            // Example approach: you could look up user_id in DB, then do scoring checks.
            // Here, we just do dummy checks:
            let dummy_spam_score = 0.0;
            let dummy_quality_score = 1.0;
            if dummy_spam_score >= spam_cut || dummy_quality_score < quality_min {
                purge_list.push(user_name);
            }
        }
        if purge_list.is_empty() {
            return;
        }

        // 2) Remove those users' messages from user_map
        for uname in purge_list {
            if let Some(mut q) = self.user_map.get_mut(&uname) {
                q.clear();
            }
        }

        // 3) Rebuild ring for those that remain
        {
            let mut guard = self.global.write().await;
            let capacity = guard.capacity;
            let start_idx = guard.start_idx;
            let total_count = guard.total_count;

            let mut keep = Vec::with_capacity(total_count);
            for i in 0..total_count {
                let pos = (start_idx + i) % capacity;
                if let Some(msg) = guard.messages[pos].take() {
                    // If the user_map for this name is empty, skip it
                    if let Some(q) = self.user_map.get(&msg.user_name) {
                        if q.is_empty() {
                            // skip
                            continue;
                        }
                    }
                    keep.push(Some(msg));
                }
            }
            guard.messages.fill(None);
            guard.start_idx = 0;
            guard.end_idx = 0;
            guard.total_count = 0;
            for slot in keep {
                if let Some(m) = slot {
                    guard.push(m);
                }
            }
            self.total_in_buffer.store(guard.total_count, Ordering::Release);
        }

        self.remove_stale_indices();
    }

    /// Remove from each user's queue those ring indices that have been overwritten or trimmed.
    fn remove_stale_indices(&self) {
        let (start_idx, total_count, cap) = {
            if let Ok(g) = self.global.try_read() {
                (g.start_idx, g.total_count, g.capacity)
            } else {
                // if try_read fails, just skip
                return;
            }
        };

        if total_count == 0 {
            for mut entry in self.user_map.iter_mut() {
                entry.clear();
            }
            self.total_in_buffer.store(0, Ordering::Release);
            return;
        }

        let end_pos = (start_idx + total_count) % cap;
        let in_range = move |ring_idx: usize| {
            if start_idx < end_pos {
                ring_idx >= start_idx && ring_idx < end_pos
            } else {
                ring_idx >= start_idx || ring_idx < end_pos
            }
        };

        for mut entry in self.user_map.iter_mut() {
            entry.retain(|&idx| in_range(idx));
        }

        if let Ok(g2) = self.global.try_read() {
            self.total_in_buffer.store(g2.total_count, Ordering::Release);
        }
    }

    /// Provide a get_recent_messages(...) method for retrieving messages with optional token_limit
    pub async fn get_recent_messages(
        &self,
        since: DateTime<Utc>,
        token_limit: Option<usize>,
        filter_user_name: Option<&str>,
    ) -> Vec<CachedMessage> {
        let mut results = Vec::new();
        let global_guard = self.global.read().await;

        let capacity = global_guard.capacity;
        let start_idx = global_guard.start_idx;
        let total_count = global_guard.total_count;
        if total_count == 0 {
            return results;
        }

        let mut tokens_used = 0usize;

        if let Some(uname) = filter_user_name {
            if let Some(q) = self.user_map.get(uname) {
                for &idx in q.iter() {
                    if let Some(m) = global_guard.get(idx) {
                        if m.timestamp < since {
                            continue;
                        }
                        if let Some(tlimit) = token_limit {
                            if tokens_used + m.token_count > tlimit {
                                break;
                            }
                            tokens_used += m.token_count;
                        }
                        results.push(m.clone());
                    }
                }
            }
        } else {
            // Global scan from oldest to newest
            for i in 0..total_count {
                let ring_pos = (start_idx + i) % capacity;
                if let Some(msg) = global_guard.messages[ring_pos].as_ref() {
                    if msg.timestamp >= since {
                        if let Some(tlimit) = token_limit {
                            if tokens_used + msg.token_count > tlimit {
                                break;
                            }
                            tokens_used += msg.token_count;
                        }
                        results.push(msg.clone());
                    }
                }
            }
        }
        results
    }
}