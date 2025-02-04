// tests/unit/cache_tests.rs

use std::collections::HashMap;
use async_trait::async_trait;
use chrono::{Utc, Duration};
use maowbot_core::Error;
use maowbot_core::cache::{
    ChatCache, CacheConfig, TrimPolicy, CachedMessage
};
use maowbot_core::models::user_analysis::UserAnalysis;
use maowbot_core::repositories::postgres::user_analysis::UserAnalysisRepository;

#[derive(Clone, Default)]
struct MockUserAnalysisRepo {
    pub data: HashMap<String, UserAnalysis>,
}

#[async_trait]
impl UserAnalysisRepository for MockUserAnalysisRepo {
    async fn create_analysis(&self, _analysis: &UserAnalysis) -> Result<(), Error> {
        Ok(())
    }
    async fn get_analysis(&self, user_id: &str) -> Result<Option<UserAnalysis>, Error> {
        Ok(self.data.get(user_id).cloned())
    }
    async fn update_analysis(&self, _analysis: &UserAnalysis) -> Result<(), Error> {
        Ok(())
    }
}

#[tokio::test]
async fn test_add_and_retrieve_messages() -> Result<(), Error> {
    let analysis_repo = MockUserAnalysisRepo::default();
    let mut cache = ChatCache::new(
        analysis_repo,
        CacheConfig {
            trim_policy: TrimPolicy {
                max_age_seconds: Some(86400),
                spam_score_cutoff: None,
                max_total_messages: None,
                max_messages_per_user: None,
                min_quality_score: None,
            }
        }
    );

    let now = Utc::now();
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
        timestamp: now + Duration::seconds(10),
        token_count: 3,
    };
    cache.add_message(msg2.clone()).await;

    let since = now - Duration::seconds(3600);
    let retrieved = cache.get_recent_messages(since, None, None);
    assert_eq!(retrieved.len(), 2);
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
                max_age_seconds: Some(3600),
                spam_score_cutoff: None,
                max_total_messages: None,
                max_messages_per_user: None,
                min_quality_score: None,
            }
        }
    );

    let now = Utc::now();
    let msg_old = CachedMessage {
        platform: "twitch_helix".to_string(),
        channel: "chanA".to_string(),
        user_id: "user_old".to_string(),
        text: "old text".to_string(),
        timestamp: now - Duration::hours(2),
        token_count: 5,
    };
    cache.add_message(msg_old).await;

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
    assert_eq!(all_messages.len(), 1);
    assert_eq!(all_messages[0].text, "new text");

    Ok(())
}

#[tokio::test]
async fn test_token_limit() -> Result<(), Error> {
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

    let now = Utc::now();
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

    let since = now - Duration::hours(1);

    let retrieved = cache.get_recent_messages(since, Some(9), None);
    assert_eq!(retrieved.len(), 1);
    assert_eq!(retrieved[0].text, "cccccc");

    let retrieved2 = cache.get_recent_messages(since, Some(10), None);
    assert_eq!(retrieved2.len(), 2);
    assert_eq!(retrieved2[0].text, "cccccc");
    assert_eq!(retrieved2[1].text, "bbbb");

    Ok(())
}