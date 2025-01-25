use maowbot::{Database, models::{User, Platform, PlatformIdentity}};
use chrono::Utc;
use std::{env, fs};
use serde_json::json;
use uuid::Uuid;

#[tokio::test]
async fn test_database_connection() -> anyhow::Result<()> {
    // Print current directory for debugging
    println!("Current Dir: {:?}", env::current_dir()?);

    // Determine the test database path
    let db_path = env::current_dir()?.join("data/test.db");
    println!("Test database path: {:?}", db_path);

    // Ensure the parent directory exists
    if let Some(parent) = db_path.parent() {
        if !parent.exists() {
            println!("Creating parent directory: {:?}", parent);
            fs::create_dir_all(parent)?;
        }
    }

    // Remove any existing test database to ensure a clean slate
    if db_path.exists() {
        println!("Removing existing test database: {:?}", db_path);
        fs::remove_file(&db_path)?;
    }

    // Initialize the database
    let db = Database::new(db_path.to_str().unwrap()).await?;
    println!("Database initialized successfully!");

    // Apply migrations
    db.migrate().await?;
    println!("Database migrated successfully!");

    // Prepare user data
    let now = Utc::now().naive_utc();
    let user = User {
        user_id: "test_user".to_string(),
        global_username: None,  // <= required
        created_at: now,
        last_seen: now,
        is_active: true,
    };


    // Insert a user
    sqlx::query!(
        r#"
        INSERT INTO users (user_id, created_at, last_seen, is_active)
        VALUES (?1, ?2, ?3, ?4)
        "#,
        user.user_id,
        user.created_at,
        user.last_seen,
        user.is_active
    )
        .execute(db.pool())
        .await?;
    println!("User inserted successfully!");

    // Retrieve the user by specifying columns explicitly
    let retrieved = sqlx::query_as!(
        User,
        r#"
        SELECT user_id, global_username, created_at, last_seen, is_active
        FROM users
        WHERE user_id = ?1
        "#,
        user.user_id
    )
        .fetch_one(db.pool())
        .await?;
    println!("User retrieved successfully: {:?}", retrieved);

    // Validate the user data
    assert_eq!(user.user_id, retrieved.user_id);
    assert_eq!(user.created_at, retrieved.created_at);
    assert_eq!(user.last_seen, retrieved.last_seen);
    assert!(retrieved.is_active);

    Ok(())
}

#[tokio::test]
async fn test_migration() -> anyhow::Result<()> {
    let db = Database::new(":memory:").await?;
    db.migrate().await?;
    println!("Migrations applied successfully (in memory)!");
    Ok(())
}

#[test]
fn test_file_access() {
    let db_path = std::path::Path::new("data/test.db");
    if db_path.exists() {
        std::fs::remove_file(&db_path).unwrap();
    }
    let file = std::fs::File::create(&db_path);
    assert!(file.is_ok(), "Failed to create database file");
}

#[tokio::test]
async fn test_platform_identity() -> anyhow::Result<()> {
    // Change this line - use ":memory:" directly for in-memory database
    let db = Database::new(":memory:").await?;
    db.migrate().await?;

    // First create a user
    let user = User {
        user_id: "test_user".to_string(),
        global_username: None,
        created_at: Utc::now().naive_utc(),
        last_seen: Utc::now().naive_utc(),
        is_active: true,
    };

    sqlx::query!(
        "INSERT INTO users (user_id, created_at, last_seen, is_active) VALUES (?, ?, ?, ?)",
        user.user_id,
        user.created_at,
        user.last_seen,
        user.is_active
    )
        .execute(db.pool())
        .await?;

    // Create a platform identity
    let now = Utc::now().naive_utc();
    let platform_identity = PlatformIdentity {
        platform_identity_id: Uuid::new_v4().to_string(),
        user_id: user.user_id.clone(),
        platform: Platform::Twitch,
        platform_user_id: "twitch_123".to_string(),
        platform_username: "twitchuser".to_string(),
        platform_display_name: Some("Twitch User".to_string()),
        platform_roles: vec!["broadcaster".to_string()],
        platform_data: json!({
            "profile_image_url": "https://example.com/image.jpg"
        }),
        created_at: now,
        last_updated: now,
    };

    // Insert platform identity
    let platform_str = platform_identity.platform.to_string();
    let roles_json = serde_json::to_string(&platform_identity.platform_roles)?;
    let data_json = platform_identity.platform_data.to_string();

    sqlx::query!(
        r#"
        INSERT INTO platform_identities (
            platform_identity_id, user_id, platform, platform_user_id,
            platform_username, platform_display_name, platform_roles,
            platform_data, created_at, last_updated
        )
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
        platform_identity.platform_identity_id,
        platform_identity.user_id,
        platform_str,
        platform_identity.platform_user_id,
        platform_identity.platform_username,
        platform_identity.platform_display_name,
        roles_json,
        data_json,
        platform_identity.created_at,
        platform_identity.last_updated,
    )
        .execute(db.pool())
        .await?;

    // Test retrieving the platform identity
    let row = sqlx::query!(
        r#"
        SELECT *
        FROM platform_identities
        WHERE platform_identity_id = ?
        "#,
        platform_identity.platform_identity_id
    )
        .fetch_one(db.pool())
        .await?;

    // Simple field comparisons (these should now work without unwrap)
    assert_eq!(platform_identity.platform_identity_id, row.platform_identity_id);
    assert_eq!(platform_identity.platform_user_id, row.platform_user_id);
    assert_eq!(platform_identity.platform_username, row.platform_username);

    Ok(())
}