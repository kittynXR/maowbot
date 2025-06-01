// File: maowbot-core/tests/integration/services_tests.rs

use std::sync::Arc;
use anyhow::anyhow;
use tokio::sync::Mutex;
use tokio::time::{timeout, Duration};

use maowbot_core::{
    Error,
    auth::user_manager::DefaultUserManager,
    cache::{ChatCache, CacheConfig, TrimPolicy, CachedMessage},
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

use maowbot_core::test_utils::helpers::setup_test_database;

#[tokio::test]
async fn test_user_message_services_db() -> Result<(), Error> {
    // Use the standard Postgres test DB
    let db = setup_test_database().await?;

    // Set up repositories and the user manager
    let user_repo = UserRepository::new(db.pool().clone());
    let ident_repo = PlatformIdentityRepository::new(db.pool().clone());
    let analysis_repo = PostgresUserAnalysisRepository::new(db.pool().clone());
    let default_user_manager = DefaultUserManager::new(user_repo, ident_repo, analysis_repo);
    let user_service = UserService::new(Arc::new(default_user_manager));

    // Create the event bus
    let event_bus = Arc::new(EventBus::new());

    // Set up the chat cache with a trimming policy
    let trim_policy = TrimPolicy {
        max_age_seconds: Some(86400), // 1 day
        spam_score_cutoff: None,
        max_total_messages: Some(500),
        max_messages_per_user: None,
        min_quality_score: None,
    };
    let cache = ChatCache::new(
        PostgresUserAnalysisRepository::new(db.pool().clone()),
        CacheConfig { trim_policy },
    );

    // Wrap it in a Mutex, matching your MessageService constructor
    let chat_cache = Arc::new(Mutex::new(cache));
    let message_service = MessageService::new(chat_cache.clone(), event_bus.clone());

    // Subscribe to events
    let mut rx = event_bus.subscribe(None).await;

    // Create a user (via the user service)
    let user = user_service
        .get_or_create_user("twitch_helix", "some_twitch_id", Some("TwitchName"))
        .await?;
    assert_eq!(user.user_id.len(), 36);

    // Publish a chat message
    let msg_text = "Hello chat";
    message_service
        .process_incoming_message("twitch_helix", "channel1", &user.user_id, msg_text)
        .await?;

    // Wait up to 1 second to see if the event is published
    let maybe_event = timeout(Duration::from_secs(1), rx.recv()).await?;
    let event = maybe_event.ok_or_else(|| anyhow!("No event received"))?;

    match event {
        BotEvent::ChatMessage { platform, channel, user: msg_user, text, .. } => {
            assert_eq!(platform, "twitch_helix");
            assert_eq!(channel, "channel1");
            assert_eq!(msg_user, user.user_id);
            assert_eq!(text, msg_text);
        }
        other => panic!("Expected ChatMessage, got {:?}", other),
    }

    // Confirm that the message was cached in memory
    let since = chrono::Utc::now() - chrono::Duration::hours(1);
    let cached = message_service.get_recent_messages(since, None, None).await;
    assert_eq!(cached.len(), 1);
    assert_eq!(cached[0].text, msg_text);

    Ok(())
}