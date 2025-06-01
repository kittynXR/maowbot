// File: maowbot-core/tests/unit/cache_tests.rs

use std::collections::HashMap;
use std::sync::Arc;
use async_trait::async_trait;
use chrono::{Utc, Duration};
use uuid::Uuid;
use maowbot_core::Error;
use maowbot_core::cache::{
    ChatCache, CacheConfig, TrimPolicy, CachedMessage
};
use maowbot_common::models::user_analysis::UserAnalysis;
use maowbot_common::traits::repository_traits::UserAnalysisRepository;

/// A mock implementation of the UserAnalysisRepository trait,
/// storing data in a simple HashMap keyed by user_id.
#[derive(Clone, Default)]
struct MockUserAnalysisRepo {
    pub data: HashMap<Uuid, UserAnalysis>,
}

#[async_trait]
impl UserAnalysisRepository for MockUserAnalysisRepo {
    async fn create_analysis(&self, analysis: &UserAnalysis) -> Result<(), Error> {
        // For a real DB, you'd insert; here, we just store in a local map
        let mut cloned = analysis.clone();
        cloned.updated_at = Utc::now();
        let user_id = cloned.user_id;
        let mut me = self.clone();
        me.data.insert(user_id, cloned);
        Ok(())
    }

    async fn get_analysis(&self, user_id: Uuid) -> Result<Option<UserAnalysis>, Error> {
        Ok(self.data.get(&user_id).cloned())
    }

    async fn update_analysis(&self, analysis: &UserAnalysis) -> Result<(), Error> {
        let mut cloned = analysis.clone();
        cloned.updated_at = Utc::now();
        let user_id = cloned.user_id;
        let mut me = self.clone();
        me.data.insert(user_id, cloned);
        Ok(())
    }
}

/// Helper to build a default ChatCache with the given policy overrides.
fn build_cache(
    policy: TrimPolicy,
) -> ChatCache<MockUserAnalysisRepo> {
    let analysis_repo = MockUserAnalysisRepo::default();
    ChatCache::new(
        analysis_repo,
        CacheConfig { trim_policy: policy },
    )
}

#[tokio::test]
async fn test_add_and_retrieve_messages() -> Result<(), Error> {
    let policy = TrimPolicy {
        max_age_seconds: None,
        spam_score_cutoff: None,
        max_total_messages: Some(100),
        max_messages_per_user: None,
        min_quality_score: None,
    };
    let cache = build_cache(policy);

    let now = Utc::now();
    let user1_id = Uuid::new_v4(); // Generate proper UUID
    let msg1 = CachedMessage {
        platform: "discord".into(),
        channel: "general".into(),
        user_id: user1_id,
        user_name: "user1".into(),
        text: "Hello from user1".into(),
        timestamp: now,
        token_count: 3,
    };
    cache.add_message(msg1.clone()).await;

    let user2_id = Uuid::new_v4(); // Generate proper UUID
    let msg2 = CachedMessage {
        platform: "discord".into(),
        channel: "general".into(),
        user_id: user2_id,
        user_name: "user2".into(),
        text: "user2 checking in".into(),
        timestamp: now + Duration::seconds(5),
        token_count: 4,
    };
    cache.add_message(msg2.clone()).await;

    // Retrieve all messages since 1 hour ago
    let since = now - Duration::hours(1);
    let retrieved = cache.get_recent_messages(since, None, None).await;
    assert_eq!(retrieved.len(), 2, "Should retrieve both messages");
    // By default, messages come out in ascending order by timestamp
    assert_eq!(retrieved[0].text, "Hello from user1");
    assert_eq!(retrieved[1].text, "user2 checking in");

    Ok(())
}

#[tokio::test]
async fn test_ring_overwrites_when_full() -> Result<(), Error> {
    let policy = TrimPolicy {
        max_age_seconds: None,
        spam_score_cutoff: None,
        max_total_messages: Some(2), // ring capacity = 2
        max_messages_per_user: None,
        min_quality_score: None,
    };
    let cache = build_cache(policy);

    let now = Utc::now();
    let m1 = CachedMessage {
        platform: "test".into(),
        channel: "chan".into(),
        user_id: Uuid::new_v4(),
        user_name: "u1".into(),
        text: "first".into(),
        timestamp: now,
        token_count: 1,
    };
    let m2 = CachedMessage {
        platform: "test".into(),
        channel: "chan".into(),
        user_id: Uuid::new_v4(),
        user_name: "u2".into(),
        text: "second".into(),
        timestamp: now + Duration::seconds(5),
        token_count: 1,
    };
    let m3 = CachedMessage {
        platform: "test".into(),
        channel: "chan".into(),
        user_id: Uuid::new_v4(),
        user_name: "u3".into(),
        text: "third".into(),
        timestamp: now + Duration::seconds(10),
        token_count: 1,
    };

    cache.add_message(m1.clone()).await;
    cache.add_message(m2.clone()).await;
    cache.add_message(m3.clone()).await;

    // Now only m2 and m3 should remain in the ring
    let since = now - Duration::hours(1);
    let all = cache.get_recent_messages(since, None, None).await;
    assert_eq!(all.len(), 2, "Capacity is 2, so it overwrote the oldest");
    assert_eq!(all[0].text, "second");
    assert_eq!(all[1].text, "third");

    Ok(())
}

#[tokio::test]
async fn test_per_user_capacity() -> Result<(), Error> {
    let policy = TrimPolicy {
        max_age_seconds: None,
        spam_score_cutoff: None,
        max_total_messages: Some(10),
        max_messages_per_user: Some(2),
        min_quality_score: None,
    };
    let cache = build_cache(policy);

    let now = Utc::now();
    let user_id = Uuid::new_v4();
    
    // Add 3 messages for user "abc"
    for i in 0..3 {
        let msg = CachedMessage {
            platform: "test".into(),
            channel: "chan".into(),
            user_id,
            user_name: "abc".into(),
            text: format!("msg #{}", i),
            timestamp: now + Duration::seconds(i as i64),
            token_count: 1,
        };
        cache.add_message(msg).await;
    }

    // Because max_messages_per_user=2, only the last 2 for "abc" remain in the user's queue
    let since = now - Duration::minutes(1);
    let retrieved_abc = cache.get_recent_messages(since, None, Some("abc")).await;
    assert_eq!(retrieved_abc.len(), 2);
    assert_eq!(retrieved_abc[0].text, "msg #1");
    assert_eq!(retrieved_abc[1].text, "msg #2");

    // If we retrieve all messages (no filter_user_id), we might still see 3 in the ring
    // *except* that the first message's index was ejected from user "abc"'s queue.
    // However, the ring doesn't forcibly remove it unless capacity is exceeded or it's old.
    let all = cache.get_recent_messages(since, None, None).await;
    assert_eq!(all.len(), 3, "All remain in ring for now, but user queue is trimmed to 2");

    Ok(())
}

#[tokio::test]
async fn test_age_based_trimming_on_add() -> Result<(), Error> {
    let policy = TrimPolicy {
        max_age_seconds: Some(3600), // 1 hour
        spam_score_cutoff: None,
        max_total_messages: Some(100),
        max_messages_per_user: None,
        min_quality_score: None,
    };
    let cache = build_cache(policy);

    let now = Utc::now();
    let old_msg = CachedMessage {
        platform: "test".into(),
        channel: "chan".into(),
        user_id: Uuid::new_v4(),
        user_name: "olduser".into(),
        text: "too old".into(),
        timestamp: now - Duration::hours(2),
        token_count: 1,
    };
    // Adding an old message will cause the inline age-based trim to remove it immediately.
    cache.add_message(old_msg.clone()).await;

    let new_msg = CachedMessage {
        platform: "test".into(),
        channel: "chan".into(),
        user_id: Uuid::new_v4(),
        user_name: "newuser".into(),
        text: "newer message".into(),
        timestamp: now,
        token_count: 1,
    };
    cache.add_message(new_msg.clone()).await;

    let since = now - Duration::hours(3);
    let messages = cache.get_recent_messages(since, None, None).await;
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].text, "newer message");

    Ok(())
}

#[tokio::test]
async fn test_token_limit_stops_early() -> Result<(), Error> {
    let policy = TrimPolicy {
        max_age_seconds: None,
        spam_score_cutoff: None,
        max_total_messages: Some(10),
        max_messages_per_user: None,
        min_quality_score: None,
    };
    let cache = build_cache(policy);

    let now = Utc::now();
    let m1 = CachedMessage {
        platform: "test".into(),
        channel: "chan".into(),
        user_id: Uuid::new_v4(),
        user_name: "u1".into(),
        text: "one".into(),
        timestamp: now,
        token_count: 4,
    };
    let m2 = CachedMessage {
        platform: "test".into(),
        channel: "chan".into(),
        user_id: Uuid::new_v4(),
        user_name: "u2".into(),
        text: "two".into(),
        timestamp: now + Duration::seconds(10),
        token_count: 3,
    };
    let m3 = CachedMessage {
        platform: "test".into(),
        channel: "chan".into(),
        user_id: Uuid::new_v4(),
        user_name: "u3".into(),
        text: "three".into(),
        timestamp: now + Duration::seconds(20),
        token_count: 5,
    };

    cache.add_message(m1).await;
    cache.add_message(m2).await;
    cache.add_message(m3).await;

    let since = now - Duration::hours(1);

    // If token_limit=7, we look from newest to oldest in the ring, but the
    // code actually iterates in ascending timestamp, stopping once we exceed the limit.
    // So we gather in ascending order:
    //   (1) "one" (4 tokens so far),
    //   (2) "two" (4+3=7 tokens),
    //   (3) next is "three" which has 5 tokens => total would be 12, so we stop.
    let retrieved = cache.get_recent_messages(since, Some(7), None).await;
    assert_eq!(retrieved.len(), 2);
    assert_eq!(retrieved[0].text, "one");
    assert_eq!(retrieved[1].text, "two");

    Ok(())
}

#[tokio::test]
async fn test_trim_spammy_users() -> Result<(), Error> {
    // We'll seed the MockUserAnalysisRepo with a user that has spam_score above the cutoff
    let mut repo = MockUserAnalysisRepo::default();
    let spammy_id = Uuid::new_v4();
    let spam_user = UserAnalysis {
        user_analysis_id: Uuid::new_v4(),
        user_id: spammy_id,
        spam_score: 9.0,
        intelligibility_score: 0.5,
        quality_score: 0.1,
        horni_score: 0.2,
        ai_notes: None,
        moderator_notes: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };
    repo.data.insert(spammy_id, spam_user);

    let config = CacheConfig {
        trim_policy: TrimPolicy {
            max_age_seconds: None,
            spam_score_cutoff: Some(5.0),
            max_total_messages: Some(100),
            max_messages_per_user: None,
            min_quality_score: None,
        },
    };
    let cache = ChatCache::new(repo.clone(), config);

    let now = Utc::now();
    let spam_msg = CachedMessage {
        platform: "test".into(),
        channel: "chan".into(),
        user_id: spammy_id,
        user_name: "spammy".into(),
        text: "buy followers cheap!!!!".into(),
        timestamp: now,
        token_count: 2,
    };
    cache.add_message(spam_msg.clone()).await;

    let normal_msg = CachedMessage {
        platform: "test".into(),
        channel: "chan".into(),
        user_id: Uuid::new_v4(),
        user_name: "normal".into(),
        text: "normal user message".into(),
        timestamp: now + Duration::seconds(1),
        token_count: 2,
    };
    cache.add_message(normal_msg.clone()).await;

    // Before trim
    let since = now - Duration::hours(1);
    let all_pre = cache.get_recent_messages(since, None, None).await;
    assert_eq!(all_pre.len(), 2);

    // Now trim spammy users
    cache.trim_spammy_users().await;

    // The "spammy" user should be purged from user_map, and also removed from ring
    let all_post = cache.get_recent_messages(since, None, None).await;
    assert_eq!(all_post.len(), 1, "spammy messages removed");
    assert_eq!(all_post[0].text, "normal user message");

    // Confirm user_id=spammy is no longer in the ring or user queues
    let spam_only = cache.get_recent_messages(since, None, Some("spammy")).await;
    assert!(spam_only.is_empty());

    Ok(())
}