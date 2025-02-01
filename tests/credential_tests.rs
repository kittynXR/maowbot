// tests/credential_tests.rs

use maowbot::{Database, models::*, repositories::CredentialsRepository, repositories::sqlite::SqliteCredentialsRepository, crypto::Encryptor, Error};
use chrono::Utc;
use uuid::Uuid;
use sqlx::{Row};

async fn setup_test_db() -> (Database, Encryptor) {
    let db = Database::new(":memory:").await.unwrap();
    db.migrate().await.unwrap();

    // In production, this key would come from secure config
    let key = [0u8; 32]; // Test key
    let encryptor = Encryptor::new(&key).unwrap();

    (db, encryptor)
}

#[tokio::test]
async fn test_credential_storage() -> Result<(), Error> {
    let (db, encryptor) = setup_test_db().await;
    let repo = SqliteCredentialsRepository::new(db.pool().clone(), encryptor);

    let now = Utc::now().naive_utc();

    // Must insert a user first, due to the FOREIGN KEY constraint
    sqlx::query(
        r#"
        INSERT INTO users (user_id, created_at, last_seen, is_active)
        VALUES (?, ?, ?, ?)
        "#
    )
        .bind("test_user")
        .bind(now)
        .bind(now)
        .bind(true)
        .execute(db.pool())
        .await?;

    let test_cred = PlatformCredential {
        credential_id: "test_id".to_string(),
        platform: Platform::Twitch,
        credential_type: CredentialType::OAuth2,
        user_id: "test_user".to_string(),
        primary_token: "test_token".to_string(),
        refresh_token: Some("refresh_token".to_string()),
        additional_data: None,
        expires_at: Some(now),
        created_at: now,
        updated_at: now,
    };

    // Store credentials
    repo.store_credentials(&test_cred).await?;

    // Retrieve
    let retrieved = repo.get_credentials(&Platform::Twitch, "test_user")
        .await?
        .expect("Credentials should exist");

    assert_eq!(test_cred.credential_id, retrieved.credential_id);
    assert_eq!(test_cred.primary_token, retrieved.primary_token);
    assert_eq!(test_cred.refresh_token, retrieved.refresh_token);

    Ok(())
}
