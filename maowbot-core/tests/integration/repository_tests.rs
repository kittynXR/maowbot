// File: maowbot-core/tests/integration/repository_tests.rs

use chrono::{Utc, DateTime};
use serde_json::json;
use uuid::Uuid;
use sqlx::Executor;

use maowbot_core::{
    db::Database,
    models::{Platform, PlatformIdentity, User},
    repositories::postgres::{
        user::UserRepository,
        platform_identity::PlatformIdentityRepository,
    },
    repositories::Repository,
    Error,
};
use maowbot_core::repositories::postgres::platform_identity::PlatformIdentityRepo;
use maowbot_core::repositories::postgres::user::UserRepo;
use maowbot_core::test_utils::helpers::*;

#[tokio::test]
async fn test_user_repository() -> Result<(), Error> {
    let db = setup_test_database().await?;
    let repo = UserRepository::new(db.pool().clone());

    let now = Utc::now();
    let user = User {
        user_id: "test_user".to_string(),
        global_username: None,
        created_at: now,
        last_seen: now,
        is_active: true,
    };

    // Create
    repo.create(&user).await?;
    let retrieved = repo.get(&user.user_id).await?.expect("User should exist");
    assert_eq!(user.user_id, retrieved.user_id);

    // Update
    let mut updated_user = user.clone();
    updated_user.is_active = false;
    repo.update(&updated_user).await?;
    let retrieved = repo.get(&user.user_id).await?.expect("User should exist");
    assert!(!retrieved.is_active);

    // Delete
    repo.delete(&user.user_id).await?;
    let retrieved = repo.get(&user.user_id).await?;
    assert!(retrieved.is_none());

    Ok(())
}

#[tokio::test]
async fn test_platform_identity_repository() -> Result<(), Error> {
    let db = setup_test_database().await?;
    let repo = PlatformIdentityRepository::new(db.pool().clone());

    let now = Utc::now();
    // Insert a test user row
    sqlx::query(
        r#"INSERT INTO users (user_id, created_at, last_seen, is_active)
           VALUES ($1, $2, $2, TRUE)"#
    )
        .bind("test_user")
        .bind(now)
        .execute(db.pool())
        .await?;

    let identity = PlatformIdentity {
        platform_identity_id: Uuid::new_v4().to_string(),
        user_id: "test_user".to_string(),
        platform: Platform::Twitch,
        platform_user_id: "twitch_123".to_string(),
        platform_username: "testuser".to_string(),
        platform_display_name: Some("Test User".to_string()),
        platform_roles: vec!["broadcaster".to_string()],
        platform_data: json!({ "profile_image_url": "https://example.com/image.jpg" }),
        created_at: now,
        last_updated: now,
    };

    // Create
    repo.create(&identity).await?;
    let retrieved = repo
        .get(&identity.platform_identity_id)
        .await?
        .expect("Platform identity should exist");
    assert_eq!(identity.platform_identity_id, retrieved.platform_identity_id);

    // get_by_platform
    let by_platform = repo
        .get_by_platform(Platform::Twitch, &identity.platform_user_id)
        .await?
        .expect("Platform identity should exist");
    assert_eq!(identity.platform_identity_id, by_platform.platform_identity_id);

    // get_all_for_user
    let user_identities = repo.get_all_for_user(&identity.user_id).await?;
    assert_eq!(user_identities.len(), 1);

    // Update
    let mut updated_identity = identity.clone();
    updated_identity.platform_display_name = Some("Updated Test User".to_string());
    repo.update(&updated_identity).await?;
    let retrieved = repo
        .get(&identity.platform_identity_id)
        .await?
        .expect("Platform identity should exist");
    assert_eq!(
        retrieved.platform_display_name,
        Some("Updated Test User".to_string())
    );

    // Delete
    repo.delete(&identity.platform_identity_id).await?;
    let retrieved = repo.get(&identity.platform_identity_id).await?;
    assert!(retrieved.is_none());

    Ok(())
}