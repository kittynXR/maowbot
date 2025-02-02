// tests/repository_tests.rs

use maowbot::{Database, models::{User, Platform, PlatformIdentity}, repositories::postgres::{UserRepository, PlatformIdentityRepository}, repositories::Repository, Error};
use chrono::Utc;
use serde_json::json;
use uuid::Uuid;
use maowbot::utils::time::{to_epoch, from_epoch};

async fn setup_test_db() -> Database {
    let db = Database::new(":memory:").await.unwrap();
    db.migrate().await.unwrap();
    db
}

#[tokio::test]
async fn test_user_repository() -> Result<(), Error> {
    let db = setup_test_db().await;
    let repo = UserRepository::new(db.pool().clone());

    let now = Utc::now().naive_utc();
    let user = User {
        user_id: "test_user".to_string(),
        global_username: None,
        created_at: now,
        last_seen: now,
        is_active: true,
    };

    repo.create(&user).await?;
    let retrieved = repo.get(&user.user_id).await?.expect("User should exist");
    assert_eq!(user.user_id, retrieved.user_id);
    assert_eq!(retrieved.is_active, true);
    // Verify timestamps by converting to epoch seconds.
    assert_eq!(to_epoch(user.created_at), to_epoch(retrieved.created_at));
    assert_eq!(to_epoch(user.last_seen), to_epoch(retrieved.last_seen));

    // Update user.
    let mut updated_user = user.clone();
    updated_user.is_active = false;
    repo.update(&updated_user).await?;
    let retrieved = repo.get(&user.user_id).await?.expect("User should exist");
    assert!(!retrieved.is_active);

    repo.delete(&user.user_id).await?;
    let retrieved = repo.get(&user.user_id).await?;
    assert!(retrieved.is_none());
    Ok(())
}

#[tokio::test]
async fn test_platform_identity_repository() -> Result<(), Error> {
    let db = setup_test_db().await;
    let repo = PlatformIdentityRepository::new(db.pool().clone());

    let now = Utc::now().naive_utc();
    // Create the user first.
    sqlx::query(
        r#"INSERT INTO users (user_id, created_at, last_seen, is_active)
        VALUES (?, ?, ?, ?)"#
    )
        .bind("test_user")
        .bind(now.timestamp())
        .bind(now.timestamp())
        .bind(true)
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
        platform_data: json!({
            "profile_image_url": "https://example.com/image.jpg"
        }),
        created_at: now,
        last_updated: now,
    };

    repo.create(&identity).await?;
    let retrieved = repo.get(&identity.platform_identity_id).await?
        .expect("Platform identity should exist");
    assert_eq!(identity.platform_identity_id, retrieved.platform_identity_id);
    assert_eq!(identity.platform_user_id, retrieved.platform_user_id);

    let by_platform = repo.get_by_platform(Platform::Twitch, &identity.platform_user_id).await?
        .expect("Platform identity should exist");
    assert_eq!(identity.platform_identity_id, by_platform.platform_identity_id);

    let user_identities = repo.get_all_for_user(&identity.user_id).await?;
    assert_eq!(user_identities.len(), 1);
    assert_eq!(user_identities[0].platform_identity_id, identity.platform_identity_id);

    let mut updated_identity = identity.clone();
    updated_identity.platform_display_name = Some("Updated Test User".to_string());
    repo.update(&updated_identity).await?;
    let retrieved = repo.get(&identity.platform_identity_id).await?
        .expect("Platform identity should exist");
    assert_eq!(retrieved.platform_display_name, Some("Updated Test User".to_string()));

    repo.delete(&identity.platform_identity_id).await?;
    let retrieved = repo.get(&identity.platform_identity_id).await?;
    assert!(retrieved.is_none());

    Ok(())
}