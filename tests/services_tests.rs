// tests/services_tests.rs
use std::sync::Arc;
use anyhow::anyhow;
use tokio::sync::Mutex;
use tokio::time::{timeout, Duration};

use maowbot::{Database, auth::user_manager::DefaultUserManager, repositories::sqlite::{
    user::UserRepository,
    platform_identity::PlatformIdentityRepository,
    user_analysis::SqliteUserAnalysisRepository,
}, eventbus::{EventBus, BotEvent}, cache::{ChatCache, CacheConfig, TrimPolicy}, services::{
    user_service::UserService,
    message_service::MessageService,
}, Error};

/// Demonstrates using UserService + MessageService with an in-memory DB.
#[tokio::test]
async fn test_user_message_services_in_memory_db() -> Result<(), Error> {
    // 1) In-memory DB
    let db = Database::new(":memory:").await?;
    db.migrate().await?;

    // 2) Build repos + default user manager
    let user_repo = UserRepository::new(db.pool().clone());
    let ident_repo = PlatformIdentityRepository::new(db.pool().clone());
    let analysis_repo = SqliteUserAnalysisRepository::new(db.pool().clone());
    let default_user_manager = DefaultUserManager::new(user_repo, ident_repo, analysis_repo);

    // 3) Create user service
    let user_service = UserService::new(Arc::new(default_user_manager));

    // 4) Setup event bus + chat cache + message service
    let event_bus = Arc::new(EventBus::new());
    let trim_policy = TrimPolicy {
        max_age_seconds: Some(86400),
        spam_score_cutoff: None,
        max_total_messages: None,
        max_messages_per_user: None,
        min_quality_score: None,
    };
    let cache = ChatCache::new(
        SqliteUserAnalysisRepository::new(db.pool().clone()),
        CacheConfig { trim_policy }
    );
    let chat_cache = Arc::new(Mutex::new(cache));
    let message_service = MessageService::new(chat_cache, event_bus.clone());

    // 5) Subscribe to event bus
    let mut rx = event_bus.subscribe(None);

    // 6) Use user_service to get/create a user
    let user = user_service
        .get_or_create_user("twitch", "some_twitch_id", Some("TwitchName"))
        .await?;
    // Confirm user_id looks like a UUID or is non-empty
    assert_eq!(user.user_id.len(), 36, "Expected a UUID user_id");

    // 7) Send a new message to message_service
    message_service
        .process_incoming_message("twitch", "channel1", &user.user_id, "Hello chat")
        .await?;

    // 8) Confirm a BotEvent::ChatMessage is published
    //    - `timeout(...)` returns Result<Option<T>, Elapsed>
    //    - `rx.recv()` returns Option<BotEvent>
    // We handle both:
    let maybe_event = timeout(Duration::from_secs(1), rx.recv()).await?;
    let event = maybe_event.ok_or_else(|| anyhow!("No event received from bus"))?;
    match event {
        BotEvent::ChatMessage { platform, channel, user: user_id2, text, .. } => {
            assert_eq!(platform, "twitch");
            assert_eq!(channel, "channel1");
            assert_eq!(user_id2, user.user_id, "Should match the user ID we passed in");
            assert_eq!(text, "Hello chat");
        }
        other => panic!("Expected BotEvent::ChatMessage, got {:?}", other),
    }

    // 9) Check the in-memory cache
    let since = chrono::Utc::now().naive_utc() - chrono::Duration::hours(1);
    let cached = message_service.get_recent_messages(since, None, None).await;
    assert_eq!(cached.len(), 1, "We have exactly 1 message in memory cache");
    assert_eq!(cached[0].text, "Hello chat");

    Ok(())
}
