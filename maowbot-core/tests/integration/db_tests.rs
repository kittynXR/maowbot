// tests/db_tests.rs

use chrono::Utc;
use serde_json::json;
use sqlx::{Row};
use uuid::Uuid;

use maowbot_core::{
    db::Database,
    Error,
    models::{Platform, PlatformIdentity},
    utils::time::from_epoch,
};
use crate::test_utils::helpers::setup_test_database;

#[tokio::test]
async fn test_database_connection() -> Result<(), Error> {
    let db = setup_test_database().await?;

    let now = Utc::now().naive_utc();
    sqlx::query(
        r#"
            INSERT INTO users (user_id, created_at, last_seen, is_active)
            VALUES ($1, $2, $2, TRUE)
        "#
    )
        .bind("test_user")
        .bind(now.timestamp())
        .execute(db.pool())
        .await?;

    let row = sqlx::query(
        r#"
        SELECT user_id, global_username, created_at, last_seen, is_active
        FROM users
        WHERE user_id = $1
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
    assert!(created_at.timestamp() > 0);
    assert!(last_seen.timestamp() > 0);

    Ok(())
}

#[tokio::test]
async fn test_migration() -> Result<(), Error> {
    // Just ensure migrations run without error and the DB is valid
    let _db = setup_test_database().await?;
    Ok(())
}

#[tokio::test]
async fn test_platform_identity() -> Result<(), Error> {
    let db = setup_test_database().await?;

    let now = Utc::now().naive_utc();
    sqlx::query(
        r#"
        INSERT INTO users (user_id, created_at, last_seen, is_active)
        VALUES ($1, $2, $3, $4)
        "#
    )
        .bind("test_user")
        .bind(now.timestamp())
        .bind(now.timestamp())
        .bind(true)
        .execute(db.pool())
        .await?;

    let platform_identity_id = Uuid::new_v4().to_string();
    let platform_str = "twitch_helix";
    let roles_json = serde_json::to_string(&vec!["broadcaster"]).unwrap();
    let data_json = json!({ "profile_image_url": "https://example.com/image.jpg" }).to_string();

    sqlx::query(
        r#"
        INSERT INTO platform_identities (
            platform_identity_id, user_id, platform, platform_user_id,
            platform_username, platform_display_name, platform_roles,
            platform_data, created_at, last_updated
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
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
        .bind(now.timestamp())
        .bind(now.timestamp())
        .execute(db.pool())
        .await?;

    let row = sqlx::query(
        r#"
        SELECT platform_identity_id, platform_user_id, platform_username
        FROM platform_identities
        WHERE platform_identity_id = $1
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