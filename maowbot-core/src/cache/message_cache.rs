// File: src/cache/message_cache.rs

use std::collections::{BTreeMap, HashMap};
use chrono::{Utc, DateTime, Duration};
use crate::models::user_analysis::UserAnalysis;
use crate::repositories::postgres::user_analysis::UserAnalysisRepository;

/// Single cached chat message
#[derive(Debug, Clone)]
pub struct CachedMessage {
    pub platform: String,
    pub channel: String,
    pub user_id: String,
    pub text: String,
    pub timestamp: DateTime<Utc>,
    pub token_count: usize,
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

/// Maintains a BTreeMap of messages keyed by “timestamp” => vector of messages
pub struct ChatCache<R: UserAnalysisRepository> {
    messages_by_timestamp: BTreeMap<DateTime<Utc>, Vec<CachedMessage>>,
    total_message_count: usize,
    user_analysis_repo: R, // for spam checks
    config: CacheConfig,
}

impl<R: UserAnalysisRepository> ChatCache<R> {
    pub fn new(user_analysis_repo: R, config: CacheConfig) -> Self {
        Self {
            messages_by_timestamp: BTreeMap::new(),
            total_message_count: 0,
            user_analysis_repo,
            config,
        }
    }

    pub async fn add_message(&mut self, msg: CachedMessage) {
        self.messages_by_timestamp
            .entry(msg.timestamp)
            .or_insert_with(Vec::new)
            .push(msg);

        self.total_message_count += 1;
        self.trim_old_messages().await;
    }

    pub fn get_recent_messages(
        &self,
        since: DateTime<Utc>,
        token_limit: Option<usize>,
        filter_user_id: Option<&str>,
    ) -> Vec<CachedMessage> {
        let mut result = Vec::new();
        let mut running_tokens = 0;

        // BTreeMap is ascending by key, so we go .range(since..) to get times >= since
        // but if we want newest first, we could collect and reverse
        for (ts, bucket) in self.messages_by_timestamp.range(since..).rev() {
            for message in bucket.iter().rev() {
                if let Some(target_user_id) = filter_user_id {
                    if message.user_id != target_user_id {
                        continue;
                    }
                }
                if let Some(limit) = token_limit {
                    if running_tokens + message.token_count > limit {
                        return result;
                    }
                }
                running_tokens += message.token_count;
                result.push(message.clone());
            }
        }
        result
    }

    pub(crate) async fn trim_old_messages(&mut self) {
        let now = Utc::now();
        let policy = &self.config.trim_policy;

        // (1) Remove messages older than max_age_seconds
        if let Some(max_age) = policy.max_age_seconds {
            let cutoff = now - Duration::seconds(max_age);
            let mut to_remove = Vec::new();
            for t in self.messages_by_timestamp.keys() {
                if *t < cutoff {
                    to_remove.push(*t);
                } else {
                    break;
                }
            }
            for ts in to_remove {
                if let Some(bucket) = self.messages_by_timestamp.remove(&ts) {
                    self.total_message_count -= bucket.len();
                }
            }
        }

        // (2) If max_total_messages is set, remove from oldest until under limit
        if let Some(max_total) = policy.max_total_messages {
            while self.total_message_count > max_total {
                if let Some((&oldest_ts, bucket)) = self.messages_by_timestamp.iter().next() {
                    let removed_count = bucket.len();
                    self.messages_by_timestamp.remove(&oldest_ts);
                    self.total_message_count -= removed_count;
                } else {
                    break;
                }
            }
        }

        // (3) spam_score or min_quality_score
        if policy.spam_score_cutoff.is_some() || policy.min_quality_score.is_some() {
            let spam_cut = policy.spam_score_cutoff.unwrap_or(f32::MAX);
            let quality_min = policy.min_quality_score.unwrap_or(-9999.0);

            let mut new_map = BTreeMap::new();
            for (ts, bucket) in self.messages_by_timestamp.iter() {
                let mut keep = Vec::new();
                for msg in bucket.iter() {
                    if let Ok(Some(analysis)) = self.user_analysis_repo.get_analysis(&msg.user_id).await {
                        if analysis.spam_score >= spam_cut {
                            continue;
                        }
                        if analysis.quality_score < quality_min {
                            continue;
                        }
                    }
                    keep.push(msg.clone());
                }
                if !keep.is_empty() {
                    new_map.insert(*ts, keep);
                }
            }
            self.messages_by_timestamp = new_map;
            self.total_message_count = self.messages_by_timestamp.values().map(|v| v.len()).sum();
        }

        // (4) If we want a per-user message limit, we'd do it here
        // omitted for brevity
    }
}
