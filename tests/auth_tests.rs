// tests/auth_tests.rs
use maowbot::{auth::{
    AuthManager, AuthenticationPrompt, AuthenticationResponse,
    AuthenticationHandler,
}, models::{Platform, PlatformCredential, CredentialType}, Error};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Mutex;
use chrono::Utc;
use maowbot::platforms::discord::auth::DiscordAuthenticator;
use maowbot::repositories::CredentialsRepository;

// Make AuthenticationResponse cloneable
#[derive(Clone)]
struct TestAuthResponse {
    code: Option<String>,
    keys: Option<HashMap<String, String>>,
}

#[derive(Default)]
struct MockAuthHandler {}

#[async_trait]
impl AuthenticationHandler for MockAuthHandler {
    async fn handle_prompt(&self, prompt: AuthenticationPrompt) -> Result<AuthenticationResponse, Error> {
        match prompt {
            AuthenticationPrompt::MultipleKeys { fields, .. } => {
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

#[derive(Default)]
struct MockCredentialsRepository {
    credentials: Mutex<HashMap<(Platform, String), PlatformCredential>>,
}

#[async_trait]
impl CredentialsRepository for MockCredentialsRepository {
    async fn store_credentials(&self, cred: &PlatformCredential) -> Result<(), Error> {
        let mut creds = self.credentials.lock().unwrap();
        creds.insert(
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
        let creds = self.credentials.lock().unwrap();
        Ok(creds.get(&(platform.clone(), user_id.to_string())).cloned())
    }

    async fn update_credentials(&self, cred: &PlatformCredential) -> Result<(), Error> {
        let mut creds = self.credentials.lock().unwrap();
        if creds.contains_key(&(cred.platform.clone(), cred.user_id.clone())) {
            creds.insert(
                (cred.platform.clone(), cred.user_id.clone()),
                cred.clone()
            );
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
        let mut creds = self.credentials.lock().unwrap();
        match creds.remove(&(platform.clone(), user_id.to_string())) {
            Some(_) => Ok(()),
            None => Err(Error::Database(sqlx::Error::RowNotFound))
        }
    }
}

// Helper methods for testing
impl MockCredentialsRepository {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_credential(self, cred: PlatformCredential) -> Self {
        self.credentials.lock().unwrap().insert(
            (cred.platform.clone(), cred.user_id.clone()),
            cred
        );
        self
    }

    pub fn credentials_count(&self) -> usize {
        self.credentials.lock().unwrap().len()
    }

    pub fn contains_credential(&self, platform: Platform, user_id: &str) -> bool {
        self.credentials.lock().unwrap().contains_key(&(platform, user_id.to_string()))
    }
}

#[tokio::test]
async fn test_credential_storage_and_retrieval() -> Result<(), Error> {
    let creds_repo = MockCredentialsRepository::new();

    // First create a test credential
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

    // Test storage
    creds_repo.store_credentials(&test_cred).await?;
    assert_eq!(creds_repo.credentials_count(), 1);
    assert!(creds_repo.contains_credential(Platform::Twitch, "test_user"));

    let retrieved = creds_repo.get_credentials(&Platform::Twitch, "test_user").await?;
    assert!(retrieved.is_some());

    let retrieved_cred = retrieved.unwrap();
    assert_eq!(retrieved_cred.credential_id, test_cred.credential_id);
    assert_eq!(retrieved_cred.primary_token, test_cred.primary_token);

    Ok(())
}

#[tokio::test]
async fn test_invalid_platform() {
    let auth_handler = MockAuthHandler::default();
    let creds_repo = MockCredentialsRepository::default();

    let mut auth_manager = AuthManager::new(
        Box::new(creds_repo),
        Box::new(auth_handler),
    );

    // Try to authenticate with unregistered platform
    let result = auth_manager.authenticate_platform(Platform::VRChat).await;
    assert!(result.is_err());
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

    // Test expired token refresh
    let _expired_cred = PlatformCredential {
        credential_id: "test_id".to_string(),
        platform: Platform::Twitch,
        credential_type: CredentialType::OAuth2,
        user_id: "test_user".to_string(),
        primary_token: "expired_token".to_string(),
        refresh_token: None,
        additional_data: None,
        expires_at: Some(Utc::now().naive_utc()),
        created_at: Utc::now().naive_utc(),
        updated_at: Utc::now().naive_utc(),
    };

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

    auth_manager.register_authenticator(
        Platform::Discord,
        Box::new(DiscordAuthenticator::new()),
    );

    // Test Discord bot token authentication
    let result = auth_manager.authenticate_platform(Platform::Discord).await?;

    assert_eq!(result.platform, Platform::Discord);
    assert_eq!(result.credential_type, CredentialType::BearerToken);
    assert!(result.refresh_token.is_none());

    Ok(())
}