// tests/user_manager_tests.rs
mod tests {
use uuid::Uuid;

use crate::Database;
use crate::models::{Platform, User};
use crate::repositories::sqlite::{
    user::UserRepository,
    platform_identity::PlatformIdentityRepository,
    user_analysis::{SqliteUserAnalysisRepository},
};
use crate::auth::{UserManager, DefaultUserManager};
use crate::Error;

/// A helper to create an in‐memory DB, run migrations, and build a DefaultUserManager.
async fn setup_user_manager() -> Result<DefaultUserManager, Error> {
    // 1) In‐memory DB
    let db = Database::new(":memory:").await?;
    db.migrate().await?;

    // 2) Repos
    let user_repo = UserRepository::new(db.pool().clone());
    let ident_repo = PlatformIdentityRepository::new(db.pool().clone());
    let analysis_repo = SqliteUserAnalysisRepository::new(db.pool().clone());

    Ok(DefaultUserManager::new(
        user_repo,
        ident_repo,
        analysis_repo,
    ))
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
    assert!(user.last_seen.timestamp() > 0);

    // Next time we call get_or_create with same IDs,
    // we should retrieve the same user from the cache or DB
    let user2 = manager
        .get_or_create_user(Platform::Discord, &random_id, Some("testuser2"))
        .await?;

    assert_eq!(user.user_id, user2.user_id, "Should be the same user");
    Ok(())
}

#[tokio::test]
async fn test_get_or_create_user_cache_hit() -> Result<(), Error> {
    let manager = setup_user_manager().await?;

    let user_id = Uuid::new_v4().to_string(); // not used as a user_id directly, just a random platform ID
    let user = manager
        .get_or_create_user(Platform::Twitch, &user_id, Some("TwitchDude"))
        .await?;

    // This first call hits the DB. The second call is the "cache hit".
    let start = std::time::Instant::now();

    let user2 = manager
        .get_or_create_user(Platform::Twitch, &user_id, Some("NewName"))
        .await?;

    let elapsed = start.elapsed();
    // It's tricky to test "should be fast" in real code, but we can at least confirm it returns the same user
    assert_eq!(user.user_id, user2.user_id);

    // Just to confirm it didn't create a new user
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
        manager.update_user_activity(&user.user_id, Some("VRChatterNew")).await?;

        let updated_user = manager
            .get_or_create_user(Platform::VRChat, "vrchat_123", None)
            .await?;

        assert!(updated_user.last_seen > old_seen, "Should have a more recent last_seen");
        assert_eq!(updated_user.global_username, Some("VRChatterNew".to_string()));

        Ok(())
    }


#[tokio::test]
async fn test_cache_ttl_expiration() -> Result<(), Error> {
    let manager = setup_user_manager().await?;

    // Insert user in the cache by calling get_or_create_user
    let user = manager
        .get_or_create_user(Platform::Discord, "some_discord_id", Some("DiscordTest"))
        .await?;

    // Force the last_access to 25 hours ago
    let changed = manager
        .test_force_last_access(Platform::Discord, "some_discord_id", 25)
        .await;
    assert!(changed, "Should have updated the existing cache entry");

    // Now calling get_or_create_user should prune that stale entry first,
    // then re-insert. We'll confirm it's still the same DB user though:
    let user2 = manager
        .get_or_create_user(Platform::Discord, "some_discord_id", Some("DiscordTest2"))
        .await?;

    // Both user + user2 should be the same DB user, but the old cache entry was removed internally.
    assert_eq!(user.user_id, user2.user_id);
    Ok(())
}
}