// tests/db_tests.rs

use maowbot::{Database, models::{User, Platform, PlatformIdentity}, Error};
use chrono::Utc;
use std::{env, fs};
use serde_json::json;
use uuid::Uuid;
use sqlx::{Row, Error as SqlxError};
use maowbot::utils::time::{from_epoch};

#[tokio::test]
async fn test_database_connection() -> Result<(), Error> {
    // Build a test database file path.
    let db_path = env::current_dir()?.join("data/test.db");
    if db_path.exists() {
        fs::remove_file(&db_path)?;
    }

    let db = Database::new(db_path.to_str().unwrap()).await?;
    db.migrate().await?;

    let now = Utc::now().naive_utc();
    // Insert a user, storing timestamps as epoch seconds.
    sqlx::query(
        r#"
        INSERT INTO users (user_id, created_at, last_seen, is_active)
        VALUES (?, ?, ?, ?)
        "#
    )
        .bind("test_user")
        .bind(now.timestamp())  // store as INTEGER
        .bind(now.timestamp())
        .bind(true)
        .execute(db.pool())
        .await?;

    // Fetch the user and convert the stored epoch integers back to NaiveDateTime.
    let row = sqlx::query(
        r#"
        SELECT user_id, global_username, created_at, last_seen, is_active
        FROM users
        WHERE user_id = ?
        "#
    )
        .bind("test_user")
        .fetch_one(db.pool())
        .await?;

    let created_epoch: i64 = row.try_get("created_at")?;
    let last_seen_epoch: i64 = row.try_get("last_seen")?;
    let created_at = from_epoch(created_epoch);
    let last_seen = from_epoch(last_seen_epoch);

    assert_eq!(row.try_get::<String, _>("user_id")?, "test_user");
    // Optionally, you can compare the converted NaiveDateTime values with your original timestamps.

    Ok(())
}

#[tokio::test]
async fn test_migration() -> Result<(), Error> {
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
async fn test_platform_identity() -> Result<(), Error> {
    let db = Database::new(":memory:").await?;
    db.migrate().await?;

    // First create a user
    let now = Utc::now().naive_utc();
    sqlx::query(
        "INSERT INTO users (user_id, created_at, last_seen, is_active) VALUES (?, ?, ?, ?)"
    )
        .bind("test_user")
        .bind(now)
        .bind(now)
        .bind(true)
        .execute(db.pool())
        .await?;

    // Create a platform identity
    let platform_identity_id = Uuid::new_v4().to_string();
    let platform_str = "twitch";
    let roles_json = serde_json::to_string(&vec!["broadcaster"]).unwrap();
    let data_json = json!({ "profile_image_url": "https://example.com/image.jpg" }).to_string();

    sqlx::query(
        r#"
        INSERT INTO platform_identities (
            platform_identity_id, user_id, platform, platform_user_id,
            platform_username, platform_display_name, platform_roles,
            platform_data, created_at, last_updated
        )
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#
    )
        .bind(&platform_identity_id)
        .bind("test_user")
        .bind(platform_str)
        .bind("twitch_123")
        .bind("twitchuser")
        .bind("Twitch User")
        .bind(roles_json)
        .bind(data_json)
        .bind(now)
        .bind(now)
        .execute(db.pool())
        .await?;

    // Verify
    let row = sqlx::query(
        r#"
        SELECT platform_identity_id, platform_user_id, platform_username
        FROM platform_identities
        WHERE platform_identity_id = ?
        "#
    )
        .bind(&platform_identity_id)
        .fetch_one(db.pool())
        .await?;

    let fetched_id: String = row.try_get("platform_identity_id")?;
    let fetched_puid: String = row.try_get("platform_user_id")?;
    let fetched_uname: String = row.try_get("platform_username")?;

    assert_eq!(fetched_id, platform_identity_id);
    assert_eq!(fetched_puid, "twitch_123");
    assert_eq!(fetched_uname, "twitchuser");

    Ok(())
}
