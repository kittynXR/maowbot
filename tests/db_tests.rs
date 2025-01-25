// tests/db_tests.rs

use maowbot::{Database, models::{User, Platform, PlatformIdentity}};
use chrono::Utc;
use std::{env, fs};
use serde_json::json;
use uuid::Uuid;

#[tokio::test]
async fn test_database_connection() -> anyhow::Result<()> {
    // For demonstration only — in CI, you might prefer an in-memory DB.
    let db_path = env::current_dir()?.join("data/test.db");

    // Remove any existing test database to ensure a clean slate
    if db_path.exists() {
        fs::remove_file(&db_path)?;
    }

    let db = Database::new(db_path.to_str().unwrap()).await?;
    db.migrate().await?;

    // Insert a user
    let now = Utc::now().naive_utc();
    sqlx::query!(
        r#"
        INSERT INTO users (user_id, created_at, last_seen, is_active)
        VALUES (?, ?, ?, ?)
        "#,
        "test_user",
        now,
        now,
        true
    )
        .execute(db.pool())
        .await?;

    // Retrieve the user
    let retrieved = sqlx::query_as!(
        User,
        r#"
        SELECT user_id, global_username, created_at, last_seen, is_active
        FROM users
        WHERE user_id = ?
        "#,
        "test_user"
    )
        .fetch_one(db.pool())
        .await?;

    assert_eq!(retrieved.user_id, "test_user");
    assert!(retrieved.is_active);

    Ok(())
}

#[tokio::test]
async fn test_migration() -> anyhow::Result<()> {
    let db = Database::new(":memory:").await?;
    db.migrate().await?;
    Ok(())
}

/// Basic file-access test (non-async)
#[test]
fn test_file_access() {
    let db_path = std::path::Path::new("data/test.db");
    if db_path.exists() {
        std::fs::remove_file(&db_path).unwrap();
    }
    let file = std::fs::File::create(&db_path);
    assert!(file.is_ok(), "Failed to create test database file");
}

#[tokio::test]
async fn test_platform_identity() -> anyhow::Result<()> {
    let db = Database::new(":memory:").await?;
    db.migrate().await?;

    // First create a user
    let now = Utc::now().naive_utc();
    sqlx::query!(
        "INSERT INTO users (user_id, created_at, last_seen, is_active) VALUES (?, ?, ?, ?)",
        "test_user",
        now,
        now,
        true
    )
        .execute(db.pool())
        .await?;

    // Create a platform identity
    let platform_identity_id = Uuid::new_v4().to_string();
    let platform_str = "twitch";
    let roles_json = serde_json::to_string(&vec!["broadcaster"])?;
    let data_json = json!({ "profile_image_url": "https://example.com/image.jpg" }).to_string();

    sqlx::query!(
        r#"
        INSERT INTO platform_identities (
            platform_identity_id, user_id, platform, platform_user_id,
            platform_username, platform_display_name, platform_roles,
            platform_data, created_at, last_updated
        )
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
        platform_identity_id,
        "test_user",
        platform_str,
        "twitch_123",
        "twitchuser",
        "Twitch User",
        roles_json,
        data_json,
        now,
        now
    )
        .execute(db.pool())
        .await?;

    // Verify
    let row = sqlx::query!(
        r#"
        SELECT platform_identity_id, platform_user_id, platform_username
        FROM platform_identities
        WHERE platform_identity_id = ?
        "#,
        platform_identity_id
    )
        .fetch_one(db.pool())
        .await?;

    assert_eq!(row.platform_identity_id, platform_identity_id);
    assert_eq!(row.platform_user_id, "twitch_123");
    assert_eq!(row.platform_username, "twitchuser");

    Ok(())
}
