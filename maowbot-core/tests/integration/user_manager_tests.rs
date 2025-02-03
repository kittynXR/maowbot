// tests/integration/user_manager_tests.rs

use uuid::Uuid;
use maowbot_core::{
    auth::{DefaultUserManager, UserManager},
    Error,
    models::{Platform, User},
    repositories::postgres::{
        user::UserRepository,
        platform_identity::PlatformIdentityRepository,
        user_analysis::PostgresUserAnalysisRepository,
    },
    db::Database,
};
use crate::test_utils::{create_test_db_pool, clean_database};

/// Creates a test Postgres DB pool, cleans it, runs migrations,
/// and returns a fully ready DefaultUserManager for testing.
async fn setup_user_manager() -> Result<DefaultUserManager, Error> {
    let pool = create_test_db_pool().await?;
    clean_database(&pool).await?;

    let db = Database::from_pool(pool.clone());
    db.migrate().await?;

    let user_repo = UserRepository::new(db.pool().clone());
    let ident_repo = PlatformIdentityRepository::new(db.pool().clone());
    let analysis_repo = PostgresUserAnalysisRepository::new(db.pool().clone());

    Ok(DefaultUserManager::new(user_repo, ident_repo, analysis_repo))
}

#[tokio::test]
async fn test_get_or_create_user() -> Result<(), Error> {
    let manager = setup_user_manager().await?;
    let random_id = Uuid::new_v4().to_string();

    let user = manager
        .get_or_create_user(Platform::Discord, &random_id, Some("testuser"))
        .await?;
    assert!(!user.user_id.is_empty());
    assert!(user.is_active);

    let user2 = manager
        .get_or_create_user(Platform::Discord, &random_id, Some("testuser2"))
        .await?;
    assert_eq!(user.user_id, user2.user_id, "Should fetch same user from cache/DB");

    Ok(())
}

#[tokio::test]
async fn test_get_or_create_user_cache_hit() -> Result<(), Error> {
    let manager = setup_user_manager().await?;
    let user_id = Uuid::new_v4().to_string();

    let user = manager
        .get_or_create_user(Platform::Twitch, &user_id, Some("TwitchDude"))
        .await?;

    let user2 = manager
        .get_or_create_user(Platform::Twitch, &user_id, Some("NewName"))
        .await?;
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

    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    manager
        .update_user_activity(&user.user_id, Some("VRChatterNew"))
        .await?;

    let updated_user = manager
        .get_or_create_user(Platform::VRChat, "vrchat_123", None)
        .await?;

    assert!(updated_user.last_seen > old_seen);
    assert_eq!(updated_user.global_username, Some("VRChatterNew".to_string()));

    Ok(())
}

#[tokio::test]
async fn test_cache_ttl_expiration() -> Result<(), Error> {
    let manager = setup_user_manager().await?;
    let platform_user_id = "some_discord_id";

    let user = manager
        .get_or_create_user(Platform::Discord, platform_user_id, Some("DiscordTest"))
        .await?;

    let changed = manager
        .test_force_last_access(Platform::Discord, platform_user_id, 25)
        .await;
    assert!(changed, "Should have updated the cache entry to 25h old");

    let user2 = manager
        .get_or_create_user(Platform::Discord, platform_user_id, Some("DiscordTest2"))
        .await?;

    assert_eq!(user.user_id, user2.user_id, "Should be same DB user, old cache entry was pruned");

    Ok(())
}