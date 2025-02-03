// File: maowbot-core/tests/integration/user_manager_tests.rs

use uuid::Uuid;
use maowbot_core::{
    auth::{DefaultUserManager, UserManager},
    models::Platform,
    repositories::postgres::{
        user::UserRepository,
        platform_identity::PlatformIdentityRepository,
        user_analysis::PostgresUserAnalysisRepository,
    },
    db::Database,
    Error,
};
use crate::test_utils::{create_test_db_pool, clean_database};

#[tokio::test]
async fn test_get_or_create_user() -> Result<(), Error> {
    // 1) Create test pool + clean it
    let pool = create_test_db_pool().await?;
    clean_database(&pool).await?;

    // 2) Wrap in Database, run migrations
    let db = Database::from_pool(pool.clone());
    db.migrate().await?;

    // 3) Repositories + manager
    let user_repo = UserRepository::new(pool.clone());
    let ident_repo = PlatformIdentityRepository::new(pool.clone());
    let analysis_repo = PostgresUserAnalysisRepository::new(pool.clone());
    let user_manager = DefaultUserManager::new(user_repo, ident_repo, analysis_repo);

    // 4) Test
    let random_id = Uuid::new_v4().to_string();
    let user = user_manager
        .get_or_create_user(Platform::Discord, &random_id, Some("testuser"))
        .await?;

    assert!(!user.user_id.is_empty(), "Should create a new user_id");
    Ok(())
}

#[tokio::test]
async fn test_user_cache_expiration() -> Result<(), Error> {
    // This time we use the “setup_test_database()” convenience if you prefer
    let db = crate::test_utils::setup_test_database().await?;

    // 1) Build repos + user_manager
    let user_repo = maowbot_core::repositories::postgres::user::UserRepository::new(db.pool().clone());
    let ident_repo = maowbot_core::repositories::postgres::platform_identity::PlatformIdentityRepository::new(db.pool().clone());
    let analysis_repo = maowbot_core::repositories::postgres::user_analysis::PostgresUserAnalysisRepository::new(db.pool().clone());
    let manager = DefaultUserManager::new(user_repo, ident_repo, analysis_repo);

    // 2) Insert user to test cache expiration
    let user_id = Uuid::new_v4().to_string();
    manager
        .get_or_create_user(Platform::Twitch, &user_id, Some("TwitchDude"))
        .await?;

    // Force last_access
    let changed = manager.test_force_last_access(Platform::Twitch, &user_id, 25).await;
    assert!(changed);

    // Next call should prune + re-create
    let user2 = manager
        .get_or_create_user(Platform::Twitch, &user_id, Some("NewName"))
        .await?;
    assert!(!user2.user_id.is_empty());
    Ok(())
}