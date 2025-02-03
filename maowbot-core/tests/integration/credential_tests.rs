// tests/credential_tests.rs
use chrono::Utc;
use sqlx::Executor;
use uuid::Uuid;

use maowbot_core::{
    crypto::Encryptor,
    models::{CredentialType, Platform, PlatformCredential},
    repositories::{CredentialsRepository},
    repositories::postgres::PostgresCredentialsRepository,
    Error,
};

use maowbot_core::test_utils::helpers::*;

#[tokio::test]
async fn test_credential_storage() -> Result<(), Error> {
    let db = setup_test_database().await?;

    // We still need an Encryptor for the credential repository
    let key = [0u8; 32]; // test key
    let encryptor = Encryptor::new(&key)?;

    let repo = PostgresCredentialsRepository::new(db.pool().clone(), encryptor);

    let now = Utc::now().naive_utc();

    // Create a user row so FKs pass
    sqlx::query(
        r#"
            INSERT INTO users (user_id, created_at, last_seen, is_active)
            VALUES ($1, $2, $2, $3)
        "#
    )
        .bind("test_user")
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

    repo.store_credentials(&test_cred).await?;

    let retrieved = repo
        .get_credentials(&Platform::Twitch, "test_user")
        .await?
        .expect("Credentials should exist");

    assert_eq!(test_cred.credential_id, retrieved.credential_id);
    assert_eq!(test_cred.primary_token, retrieved.primary_token);
    assert_eq!(test_cred.refresh_token, retrieved.refresh_token);

    Ok(())
}