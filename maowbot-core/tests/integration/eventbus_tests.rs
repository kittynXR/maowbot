// tests/integration/eventbus_tests.rs
use std::sync::Arc;
use uuid::Uuid;

use maowbot_core::{
    auth::{DefaultUserManager, UserManager},
    db::Database,
    eventbus::EventBus,
    models::Platform,
    repositories::postgres::{
        platform_identity::PlatformIdentityRepository,
        user::UserRepository,
        user_analysis::PostgresUserAnalysisRepository,
    },
    Error,
};

use maowbot_core::test_utils::helpers::*;

/// Replaces the old custom code with our helper-based approach.
async fn setup_user_manager() -> Result<DefaultUserManager, Error> {
    let db = setup_test_database().await?;
    let user_repo = UserRepository::new(db.pool().clone());
    let ident_repo = PlatformIdentityRepository::new(db.pool().clone());
    let analysis_repo = PostgresUserAnalysisRepository::new(db.pool().clone());

    Ok(DefaultUserManager::new(user_repo, ident_repo, analysis_repo))
}

#[tokio::test]
async fn test_get_or_create_user_new() -> Result<(), Error> {
    let manager = setup_user_manager().await?;

    // A random new platform_user_id
    let random_id = Uuid::new_v4().to_string();

    // This user doesn’t exist => should be created
    let user = manager
        .get_or_create_user(Platform::Discord, &random_id, Some("testuser"))
        .await?;
    assert!(!user.user_id.is_empty(), "Should have a new user_id");
    assert!(user.is_active);

    // Next time with same IDs => same user from cache/DB
    let user2 = manager
        .get_or_create_user(Platform::Discord, &random_id, Some("testuser2"))
        .await?;

    assert_eq!(user.user_id, user2.user_id, "Should be the same user");
    Ok(())
}

#[tokio::test]
async fn test_get_or_create_user_cache_hit() -> Result<(), Error> {
    let manager = setup_user_manager().await?;

    let user_id = Uuid::new_v4().to_string(); // random platform ID
    let user = manager
        .get_or_create_user(Platform::Twitch, &user_id, Some("TwitchDude"))
        .await?;

    // The second call should be a "cache hit"
    let user2 = manager
        .get_or_create_user(Platform::Twitch, &user_id, Some("NewName"))
        .await?;

    // Confirm it returns the same user ID
    assert_eq!(user.user_id, user2.user_id);

    Ok(())
}

#[tokio::test]
async fn test_update_user_activity() -> Result<(), Error> {
    let manager = setup_user_manager().await?;
    let user = manager
        .get_or_create_user(Platform::VRChat, "vrchat_123", Some("VRChatter"))
        .await?;
    let old_seen = user.last_seen;

    // Wait 1 second
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    // Update with new username
    manager
        .update_user_activity(&user.user_id, Some("VRChatterNew"))
        .await?;

    // Retrieve again
    let updated_user = manager
        .get_or_create_user(Platform::VRChat, "vrchat_123", None)
        .await?;

    assert!(
        updated_user.last_seen > old_seen,
        "Should have a more recent last_seen"
    );
    assert_eq!(
        updated_user.global_username,
        Some("VRChatterNew".to_string())
    );

    Ok(())
}

#[tokio::test]
async fn test_cache_ttl_expiration() -> Result<(), Error> {
    let manager = setup_user_manager().await?;

    // Insert user in the cache
    let user = manager
        .get_or_create_user(Platform::Discord, "some_discord_id", Some("DiscordTest"))
        .await?;

    // Force last_access to 25 hours ago => simulating stale
    let changed = manager
        .test_force_last_access(Platform::Discord, "some_discord_id", 25)
        .await;
    assert!(changed, "Should have updated the existing cache entry");

    // Next call => should prune stale cache entry, then re-insert
    let user2 = manager
        .get_or_create_user(Platform::Discord, "some_discord_id", Some("DiscordTest2"))
        .await?;

    // They should be the same DB user, but the old cache entry was removed internally
    assert_eq!(user.user_id, user2.user_id);
    Ok(())
}
