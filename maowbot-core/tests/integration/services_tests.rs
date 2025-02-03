// tests/integration/services_tests.rs

use std::sync::Arc;
use anyhow::anyhow;
use tokio::sync::Mutex;
use tokio::time::{timeout, Duration};

use maowbot_core::{
    Error,
    auth::user_manager::DefaultUserManager,
    cache::{ChatCache, CacheConfig, TrimPolicy},
    db::Database,
    eventbus::{EventBus, BotEvent},
    repositories::postgres::{
        user::UserRepository,
        platform_identity::PlatformIdentityRepository,
        user_analysis::PostgresUserAnalysisRepository,
    },
    services::{
        user_service::UserService,
        message_service::MessageService,
    },
};

use crate::test_utils::helpers::setup_test_database;

#[tokio::test]
async fn test_user_message_services_db() -> Result<(), Error> {
    // Replaces the old ":memory:" approach with our standard Postgres test DB
    let db = setup_test_database().await?;

    // Repositories + manager
    let user_repo = UserRepository::new(db.pool().clone());
    let ident_repo = PlatformIdentityRepository::new(db.pool().clone());
    let analysis_repo = PostgresUserAnalysisRepository::new(db.pool().clone());
    let default_user_manager = DefaultUserManager::new(user_repo, ident_repo, analysis_repo);
    let user_service = UserService::new(Arc::new(default_user_manager));

    // Event bus
    let event_bus = Arc::new(EventBus::new());

    // Chat cache with trimming policy
    let trim_policy = TrimPolicy {
        max_age_seconds: Some(86400),
        spam_score_cutoff: None,
        max_total_messages: None,
        max_messages_per_user: None,
        min_quality_score: None,
    };
    let cache = ChatCache::new(
        PostgresUserAnalysisRepository::new(db.pool().clone()),
        CacheConfig { trim_policy },
    );
    let chat_cache = Arc::new(Mutex::new(cache));
    let message_service = MessageService::new(chat_cache, event_bus.clone());

    // Subscribe to events
    let mut rx = event_bus.subscribe(None);

    // Create user (via user_service)
    let user = user_service
        .get_or_create_user("twitch_helix", "some_twitch_id", Some("TwitchName"))
        .await?;
    assert_eq!(user.user_id.len(), 36);

    // Publish a chat message
    message_service
        .process_incoming_message("twitch_helix", "channel1", &user.user_id, "Hello chat")
        .await?;

    // Wait up to 1s to see if the event is published
    let maybe_event = timeout(Duration::from_secs(1), rx.await.recv()).await?;
    let event = maybe_event.ok_or_else(|| anyhow!("No event received"))?;

    match event {
        BotEvent::ChatMessage { platform, channel, user: msg_user, text, .. } => {
            assert_eq!(platform, "twitch_helix");
            assert_eq!(channel, "channel1");
            assert_eq!(msg_user, user.user_id);
            assert_eq!(text, "Hello chat");
        }
        other => panic!("Expected ChatMessage, got {:?}", other),
    }

    // Confirm message was cached
    let since = chrono::Utc::now().naive_utc() - chrono::Duration::hours(1);
    let cached = message_service.get_recent_messages(since, None, None).await;
    assert_eq!(cached.len(), 1);
    assert_eq!(cached[0].text, "Hello chat");

    Ok(())
}