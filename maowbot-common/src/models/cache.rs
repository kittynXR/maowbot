use chrono::{DateTime, Utc};

/// Single cached chat message
#[derive(Debug, Clone)]
pub struct CachedMessage {
    pub platform: String,
    pub channel: String,
    pub user_name: String,
    pub text: String,
    pub timestamp: DateTime<Utc>,
    pub token_count: usize,
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