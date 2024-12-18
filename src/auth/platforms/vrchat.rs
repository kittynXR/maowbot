// src/auth/platforms/vrchat.rs
use crate::auth::{AuthenticationPrompt, AuthenticationResponse, PlatformAuthenticator};
use crate::models::{Platform, PlatformCredential, CredentialType};
use crate::Error;
use async_trait::async_trait;
use chrono::Utc;
use serde_json::json;
use uuid::Uuid;

pub struct VRChatAuthenticator {
    username: Option<String>,
    password: Option<String>,
    two_factor_method: Option<String>,
}

impl VRChatAuthenticator {
    pub fn new() -> Self {
        Self {
            username: None,
            password: None,
            two_factor_method: None,
        }
    }
}

#[async_trait]
impl PlatformAuthenticator for VRChatAuthenticator {
    async fn initialize(&mut self) -> Result<(), Error> {
        // Reset state for new authentication attempt
        self.username = None;
        self.password = None;
        self.two_factor_method = None;
        Ok(())
    }

    async fn start_authentication(&mut self) -> Result<AuthenticationPrompt, Error> {
        if self.username.is_none() || self.password.is_none() {
            Ok(AuthenticationPrompt::MultipleKeys {
                fields: vec!["username".into(), "password".into()],
                messages: vec![
                    "Enter your VRChat username".into(),
                    "Enter your VRChat password".into()
                ],
            })
        } else {
            Ok(AuthenticationPrompt::TwoFactor {
                message: "Enter your VRChat 2FA code".into()
            })
        }
    }

    async fn complete_authentication(
        &mut self,
        response: AuthenticationResponse
    ) -> Result<PlatformCredential, Error> {
        match response {
            AuthenticationResponse::MultipleKeys(creds) => {
                self.username = creds.get("username").cloned();
                self.password = creds.get("password").cloned();

                // In a real implementation, you'd make an initial auth request here
                // to verify credentials and determine if 2FA is needed

                Err(Error::Auth("2FA required".into()))
            }
            AuthenticationResponse::TwoFactor(code) => {
                let username = self.username.as_ref()
                    .ok_or_else(|| Error::Auth("Username not provided".into()))?;
                let password = self.password.as_ref()
                    .ok_or_else(|| Error::Auth("Password not provided".into()))?;

                // Here you would make the actual VRChat API call
                // with username, password, and 2FA code
                // For now, we'll create a mock credential

                Ok(PlatformCredential {
                    credential_id: Uuid::new_v4().to_string(),
                    platform: Platform::VRChat,
                    credential_type: CredentialType::Custom("vrchat_auth".into()),
                    user_id: username.clone(),
                    primary_token: "mock_auth_token".into(), // Would be real token from API
                    refresh_token: None,
                    additional_data: Some(json!({
                        "username": username,
                        "has_2fa": true
                    })),
                    expires_at: Some(Utc::now().naive_utc() + chrono::Duration::days(30)), // Example expiration
                    created_at: Utc::now().naive_utc(),
                    updated_at: Utc::now().naive_utc(),
                })
            }
            _ => Err(Error::Auth("Invalid authentication response".into()))
        }
    }

    async fn refresh(&mut self, credential: &PlatformCredential) -> Result<PlatformCredential, Error> {
        // VRChat typically requires re-authentication rather than refresh
        // You might want to trigger a new auth flow here
        Err(Error::Auth("VRChat requires re-authentication".into()))
    }

    async fn validate(&self, credential: &PlatformCredential) -> Result<bool, Error> {
        // Here you would make a test API call to VRChat
        // to verify the token is still valid

        // Example implementation:
        use reqwest::Client;

        let client = Client::new();
        let response = client.get("https://api.vrchat.cloud/api/1/auth/user")
            .header("Cookie", format!("auth={}", credential.primary_token))
            .send()
            .await?;

        Ok(response.status().is_success())
    }

    async fn revoke(&mut self, credential: &PlatformCredential) -> Result<(), Error> {
        // Here you would call VRChat's logout endpoint
        // to invalidate the current session

        use reqwest::Client;

        let client = Client::new();
        let response = client.get("https://api.vrchat.cloud/api/1/logout")
            .header("Cookie", format!("auth={}", credential.primary_token))
            .send()
            .await?;

        if response.status().is_success() {
            Ok(())
        } else {
            Err(Error::Auth("Failed to logout from VRChat".into()))
        }
    }
}