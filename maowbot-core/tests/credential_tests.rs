// tests/credential_tests.rs

use maowbot_core::{Database, models::*, repositories::CredentialsRepository, repositories::postgres::PostgresCredentialsRepository, crypto::Encryptor, Error};
use chrono::Utc;
use uuid::Uuid;
use sqlx::{Row};
use sqlx::Executor;

async fn setup_test_db() -> (Database, Encryptor) {
    // Use a proper absolute Postgres URL:
    let db_url = "postgres://maow@localhost/maowbot";
    let db = Database::new(db_url).await.unwrap();
    db.migrate().await.unwrap();

    // In production, this key would come from secure config.
    let key = [0u8; 32]; // Test key
    let encryptor = Encryptor::new(&key).unwrap();

    (db, encryptor)
}

#[tokio::test]
async fn test_credential_storage() -> Result<(), Error> {
    let (db, encryptor) = setup_test_db().await;
    let repo = PostgresCredentialsRepository::new(db.pool().clone(), encryptor);

    let now = Utc::now().naive_utc();

    sqlx::query(
        r#"
            INSERT INTO users (user_id, created_at, last_seen, is_active)
            VALUES ($1, $2, $3, $4)
            "#
        )
        .bind("test_user")
        .bind(now.timestamp()) // converting to an integer
        .bind(now.timestamp())
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
