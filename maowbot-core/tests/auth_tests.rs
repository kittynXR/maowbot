// tests/auth_tests.rs

use maowbot_core::{
    auth::{
        AuthManager, AuthenticationPrompt, AuthenticationResponse,
        AuthenticationHandler, PlatformAuthenticator,
    },
    models::{Platform, PlatformCredential, CredentialType},
    repositories::CredentialsRepository,
    Error,
};
use async_trait::async_trait;
use std::collections::HashMap;
use chrono::{Duration, Utc};
use maowbot_core::platforms::discord::auth::DiscordAuthenticator;

// ---- NEW: dashmap for concurrency
use dashmap::DashMap;

#[derive(Default)]
struct MockAuthHandler {}

#[async_trait]
impl AuthenticationHandler for MockAuthHandler {
    async fn handle_prompt(&self, prompt: AuthenticationPrompt) -> Result<AuthenticationResponse, Error> {
        match prompt {
            AuthenticationPrompt::MultipleKeys { fields, .. } => {
                // ephemeral usage of std::collections::HashMap
                let mut response = HashMap::new();
                for field in &fields {
                    response.insert(field.clone(), format!("mock_{}", field));
                }
                Ok(AuthenticationResponse::MultipleKeys(response))
            },
            _ => Err(Error::Auth("Unexpected prompt type".into())),
        }
    }
}

/// Replaced Arc<Mutex<HashMap<...>>> with a single DashMap
#[derive(Default)]
struct MockCredentialsRepository {
    credentials: DashMap<(Platform, String), PlatformCredential>,
}

#[async_trait]
impl CredentialsRepository for MockCredentialsRepository {
    async fn store_credentials(&self, cred: &PlatformCredential) -> Result<(), Error> {
        self.credentials.insert(
            (cred.platform.clone(), cred.user_id.clone()),
            cred.clone()
        );
        Ok(())
    }

    async fn get_credentials(
        &self,
        platform: &Platform,
        user_id: &str
    ) -> Result<Option<PlatformCredential>, Error> {
        if let Some(entry) = self.credentials.get(&(platform.clone(), user_id.to_string())) {
            Ok(Some(entry.value().clone()))
        } else {
            Ok(None)
        }
    }

    async fn update_credentials(&self, cred: &PlatformCredential) -> Result<(), Error> {
        let key = (cred.platform.clone(), cred.user_id.clone());
        if self.credentials.contains_key(&key) {
            self.credentials.insert(key, cred.clone());
            Ok(())
        } else {
            Err(Error::Database(sqlx::Error::RowNotFound))
        }
    }

    async fn delete_credentials(
        &self,
        platform: &Platform,
        user_id: &str
    ) -> Result<(), Error> {
        let key = (platform.clone(), user_id.to_string());
        if self.credentials.remove(&key).is_some() {
            Ok(())
        } else {
            Err(Error::Database(sqlx::Error::RowNotFound))
        }
    }

    async fn get_expiring_credentials(&self, _within: Duration) -> Result<Vec<PlatformCredential>, Error> {
        Ok(Vec::new())
    }
}

impl MockCredentialsRepository {
    pub fn new() -> Self {
        Self::default()
    }
    pub async fn credentials_count(&self) -> usize {
        self.credentials.iter().count()
    }
    pub async fn contains_credential(&self, platform: Platform, user_id: &str) -> bool {
        self.credentials.contains_key(&(platform, user_id.to_string()))
    }
}

#[tokio::test]
async fn test_credential_storage_and_retrieval() -> Result<(), Error> {
    let creds_repo = MockCredentialsRepository::new();

    let test_cred = PlatformCredential {
        credential_id: "test_id".to_string(),
        platform: Platform::Twitch,
        credential_type: CredentialType::OAuth2,
        user_id: "test_user".to_string(),
        primary_token: "test_token".to_string(),
        refresh_token: Some("refresh_token".to_string()),
        additional_data: None,
        expires_at: Some(Utc::now().naive_utc()),
        created_at: Utc::now().naive_utc(),
        updated_at: Utc::now().naive_utc(),
    };

    creds_repo.store_credentials(&test_cred).await?;
    assert_eq!(creds_repo.credentials_count().await, 1);
    assert!(creds_repo.contains_credential(Platform::Twitch, "test_user").await);

    let retrieved = creds_repo.get_credentials(&Platform::Twitch, "test_user").await?;
    assert!(retrieved.is_some());
    let retrieved_cred = retrieved.unwrap();

    assert_eq!(retrieved_cred.credential_id, test_cred.credential_id);
    Ok(())
}

#[tokio::test]
async fn test_auth_manager_with_unregistered_platform() -> Result<(), Error> {
    let auth_handler = MockAuthHandler::default();
    let creds_repo = MockCredentialsRepository::default();

    let mut auth_manager = AuthManager::new(
        Box::new(creds_repo),
        Box::new(auth_handler),
    );

    // Try to authenticate with a platform that is unregistered
    let result = auth_manager.authenticate_platform(Platform::VRChat).await;
    assert!(result.is_err());
    Ok(())
}

#[tokio::test]
async fn test_error_scenarios() -> Result<(), Error> {
    let creds_repo = MockCredentialsRepository::new();
    let auth_handler = MockAuthHandler::default();

    let mut auth_manager = AuthManager::new(
        Box::new(creds_repo),
        Box::new(auth_handler),
    );

    // Test unregistered platform
    let result = auth_manager.authenticate_platform(Platform::VRChat).await;
    assert!(matches!(result, Err(Error::Platform(_))));

    // Test refreshing with no credentials stored => should fail
    let result = auth_manager.refresh_platform_credentials(&Platform::Twitch, "test_user").await;
    assert!(matches!(result, Err(Error::Auth(_))));

    Ok(())
}

#[tokio::test]
async fn test_discord_credentials() -> Result<(), Error> {
    let creds_repo = MockCredentialsRepository::new();
    let auth_handler = MockAuthHandler::default();

    let mut auth_manager = AuthManager::new(
        Box::new(creds_repo),
        Box::new(auth_handler),
    );

    // Register DiscordAuthenticator
    auth_manager.register_authenticator(
        Platform::Discord,
        Box::new(DiscordAuthenticator::new()),
    );

    // Test Discord bot token authentication
    let result = auth_manager.authenticate_platform(Platform::Discord).await?;
    assert_eq!(result.platform, Platform::Discord);
    assert_eq!(result.credential_type, CredentialType::BearerToken);
    Ok(())
}
