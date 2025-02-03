// tests/integration/services_tests.rs

use std::sync::Arc;
use anyhow::anyhow;
use tokio::sync::Mutex;
use tokio::time::{timeout, Duration};
use maowbot_core::{Database, Error};
use maowbot_core::auth::user_manager::DefaultUserManager;
use maowbot_core::repositories::postgres::{
    user::UserRepository,
    platform_identity::PlatformIdentityRepository,
    user_analysis::PostgresUserAnalysisRepository,
};
use maowbot_core::eventbus::{EventBus, BotEvent};
use maowbot_core::cache::{ChatCache, CacheConfig, TrimPolicy};
use maowbot_core::services::{
    user_service::UserService,
    message_service::MessageService,
};

#[tokio::test]
async fn test_user_message_services_in_memory_db() -> Result<(), Error> {
    let db = Database::new(":memory:").await?;
    db.migrate().await?;

    let user_repo = UserRepository::new(db.pool().clone());
    let ident_repo = PlatformIdentityRepository::new(db.pool().clone());
    let analysis_repo = PostgresUserAnalysisRepository::new(db.pool().clone());
    let default_user_manager = DefaultUserManager::new(user_repo, ident_repo, analysis_repo);
    let user_service = UserService::new(Arc::new(default_user_manager));

    let event_bus = Arc::new(EventBus::new());
    let trim_policy = TrimPolicy {
        max_age_seconds: Some(86400),
        spam_score_cutoff: None,
        max_total_messages: None,
        max_messages_per_user: None,
        min_quality_score: None,
    };
    let cache = ChatCache::new(
        PostgresUserAnalysisRepository::new(db.pool().clone()),
        CacheConfig { trim_policy }
    );
    let chat_cache = Arc::new(Mutex::new(cache));
    let message_service = MessageService::new(chat_cache, event_bus.clone());

    let mut rx = event_bus.subscribe(None);

    let user = user_service
        .get_or_create_user("twitch_helix", "some_twitch_id", Some("TwitchName"))
        .await?;
    assert_eq!(user.user_id.len(), 36);

    message_service
        .process_incoming_message("twitch_helix", "channel1", &user.user_id, "Hello chat")
        .await?;

    let maybe_event = timeout(Duration::from_secs(1), rx.await.recv()).await?;
    let event = maybe_event.ok_or_else(|| anyhow!("No event received".into()))?;
    match event {
        BotEvent::ChatMessage { platform, channel, user: user_id2, text, .. } => {
            assert_eq!(platform, "twitch_helix");
            assert_eq!(channel, "channel1");
            assert_eq!(user_id2, user.user_id);
            assert_eq!(text, "Hello chat");
        }
        other => panic!("Expected ChatMessage, got {:?}", other),
    }

    let since = chrono::Utc::now().naive_utc() - chrono::Duration::hours(1);
    let cached = message_service.get_recent_messages(since, None, None).await;
    assert_eq!(cached.len(), 1);
    assert_eq!(cached[0].text, "Hello chat");

    Ok(())
}
