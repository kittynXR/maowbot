// tests/cache_tests.rs

use std::collections::HashMap;
use async_trait::async_trait;
use chrono::{Utc, NaiveDateTime, Duration};

use maowbot::Error;
use maowbot::cache::{
    ChatCache,
    CacheConfig,
    TrimPolicy,
    CachedMessage
};
use maowbot::models::user_analysis::UserAnalysis;
use maowbot::repositories::postgres::user_analysis::UserAnalysisRepository;

/// A mock repository that stores user_analysis in-memory only.
#[derive(Clone, Default)]
struct MockUserAnalysisRepo {
    pub data: HashMap<String, UserAnalysis>,
}

#[async_trait]
impl UserAnalysisRepository for MockUserAnalysisRepo {
    async fn create_analysis(&self, analysis: &UserAnalysis) -> Result<(), Error> {
        // we can't mutate self in an &self method unless we use interior mutability
        // for simplicity, let's just do nothing
        Ok(())
    }

    async fn get_analysis(&self, user_id: &str) -> Result<Option<UserAnalysis>, Error> {
        Ok(self.data.get(user_id).cloned())
    }

    async fn update_analysis(&self, analysis: &UserAnalysis) -> Result<(), Error> {
        // do nothing
        Ok(())
    }
}

#[tokio::test]
async fn test_add_and_retrieve_messages() -> Result<(), Error> {
    // 1) build a cache
    let analysis_repo = MockUserAnalysisRepo::default();
    let mut cache = ChatCache::new(
        analysis_repo,
        CacheConfig {
            trim_policy: TrimPolicy {
                max_age_seconds: Some(86400), // 24h
                spam_score_cutoff: None,
                max_total_messages: None,
                max_messages_per_user: None,
                min_quality_score: None,
            }
        }
    );

    // 2) add some messages
    let now = Utc::now().naive_utc();
    let msg1 = CachedMessage {
        platform: "discord".to_string(),
        channel: "channel1".to_string(),
        user_id: "user_1".to_string(),
        text: "Hello world".to_string(),
        timestamp: now,
        token_count: 5,
    };
    cache.add_message(msg1.clone()).await;

    let msg2 = CachedMessage {
        platform: "discord".to_string(),
        channel: "channel1".to_string(),
        user_id: "user_2".to_string(),
        text: "Another message".to_string(),
        timestamp: now + chrono::Duration::seconds(10),
        token_count: 3,
    };
    cache.add_message(msg2.clone()).await;

    // 3) retrieve them
    let since = now - chrono::Duration::seconds(3600); // last hour
    let retrieved = cache.get_recent_messages(since, None, None);
    assert_eq!(retrieved.len(), 2, "Should have 2 messages total");

    // Check ordering is newest-first (by default we reversed in the code)
    // So retrieved[0] should be msg2, retrieved[1] should be msg1
    assert_eq!(retrieved[0].text, "Another message");
    assert_eq!(retrieved[1].text, "Hello world");

    Ok(())
}

#[tokio::test]
async fn test_max_age_trim() -> Result<(), Error> {
    let analysis_repo = MockUserAnalysisRepo::default();
    let mut cache = ChatCache::new(
        analysis_repo,
        CacheConfig {
            trim_policy: TrimPolicy {
                max_age_seconds: Some(3600), // only keep last 1 hour
                spam_score_cutoff: None,
                max_total_messages: None,
                max_messages_per_user: None,
                min_quality_score: None,
            }
        }
    );

    let now = Utc::now().naive_utc();
    // old message from 2 hours ago
    let msg_old = CachedMessage {
        platform: "twitch_helix".to_string(),
        channel: "chanA".to_string(),
        user_id: "user_old".to_string(),
        text: "old text".to_string(),
        timestamp: now - Duration::hours(2),
        token_count: 5,
    };
    cache.add_message(msg_old).await;

    // new message
    let msg_new = CachedMessage {
        platform: "twitch_helix".to_string(),
        channel: "chanA".to_string(),
        user_id: "user_new".to_string(),
        text: "new text".to_string(),
        timestamp: now,
        token_count: 2,
    };
    cache.add_message(msg_new).await;

    let since = now - Duration::hours(24);
    let all_messages = cache.get_recent_messages(since, None, None);
    // The old one should have been trimmed because it's older than 1 hour
    assert_eq!(all_messages.len(), 1, "Only the new message should remain");
    assert_eq!(all_messages[0].text, "new text");

    Ok(())
}

#[tokio::test]
async fn test_token_limit() -> Result<(), Error> {
    // If we pass a token_limit to get_recent_messages, it should stop once it exceeds that limit
    let analysis_repo = MockUserAnalysisRepo::default();
    let mut cache = ChatCache::new(
        analysis_repo,
        CacheConfig {
            trim_policy: TrimPolicy {
                max_age_seconds: None,
                spam_score_cutoff: None,
                max_total_messages: None,
                max_messages_per_user: None,
                min_quality_score: None,
            }
        }
    );

    let now = Utc::now().naive_utc();
    // 3 messages with token_count 5, 4, 6
    let msg1 = CachedMessage {
        platform: "discord".to_string(),
        channel: "chan".to_string(),
        user_id: "u1".to_string(),
        text: "aaaaa".to_string(),
        timestamp: now,
        token_count: 5,
    };
    let msg2 = CachedMessage {
        platform: "discord".to_string(),
        channel: "chan".to_string(),
        user_id: "u2".to_string(),
        text: "bbbb".to_string(),
        timestamp: now + Duration::seconds(10),
        token_count: 4,
    };
    let msg3 = CachedMessage {
        platform: "discord".to_string(),
        channel: "chan".to_string(),
        user_id: "u3".to_string(),
        text: "cccccc".to_string(),
        timestamp: now + Duration::seconds(20),
        token_count: 6,
    };

    cache.add_message(msg1.clone()).await;
    cache.add_message(msg2.clone()).await;
    cache.add_message(msg3.clone()).await;

    // newest-first => msg3 -> msg2 -> msg1
    let since = now - Duration::hours(1);

    // Token limit 9 => we can include msg3(6 tokens) then msg2(4 tokens) would exceed 9 => stop
    // so we only get [msg3]
    let retrieved = cache.get_recent_messages(since, Some(9), None);
    assert_eq!(retrieved.len(), 1);
    assert_eq!(retrieved[0].text, "cccccc");

    // If we set limit 10 => we get msg3(6) plus msg2(4) = 10, that fits exactly
    // so 2 messages
    let retrieved2 = cache.get_recent_messages(since, Some(10), None);
    assert_eq!(retrieved2.len(), 2);
    assert_eq!(retrieved2[0].text, "cccccc"); // new
    assert_eq!(retrieved2[1].text, "bbbb");   // older

    Ok(())
}
