// File: src/platforms/vrchat/auth.rs

use async_trait::async_trait;
use chrono::Utc;
use serde_json::json;
use uuid::Uuid;

use crate::Error;
use crate::auth::{AuthenticationPrompt, AuthenticationResponse, PlatformAuthenticator};
use crate::models::{Platform, PlatformCredential, CredentialType};

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
                    "Enter your VRChat password".into(),
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

                // Potentially call VRChat here to see if 2FA is needed
                Err(Error::Auth("2FA required".into()))
            }
            AuthenticationResponse::TwoFactor(code) => {
                let username = self.username.as_ref()
                    .ok_or_else(|| Error::Auth("Username not provided".into()))?;
                let password = self.password.as_ref()
                    .ok_or_else(|| Error::Auth("Password not provided".into()))?;

                // Real VRChat API call would happen here
                Ok(PlatformCredential {
                    credential_id: Uuid::new_v4().to_string(),
                    platform: Platform::VRChat,
                    credential_type: CredentialType::Interactive2FA,
                    user_id: username.clone(),
                    primary_token: "mock_auth_token".into(),
                    refresh_token: None,
                    additional_data: Some(json!({
                        "username": username,
                        "has_2fa": true
                    })),
                    expires_at: Some(Utc::now() + chrono::Duration::days(30)),
                    created_at: Utc::now(),
                    updated_at: Utc::now(),
                    is_bot: false,
                })
            }
            _ => Err(Error::Auth("Invalid authentication response".into()))
        }
    }

    async fn refresh(&mut self, _credential: &PlatformCredential) -> Result<PlatformCredential, Error> {
        Err(Error::Auth("VRChat requires re-authentication".into()))
    }

    async fn validate(&self, credential: &PlatformCredential) -> Result<bool, Error> {
        // Example call to VRChat
        let client = reqwest::Client::new();
        let response = client.get("https://api.vrchat.cloud/api/1/auth/user")
            .header("Cookie", format!("auth={}", credential.primary_token))
            .send()
            .await?;

        Ok(response.status().is_success())
    }

    async fn revoke(&mut self, credential: &PlatformCredential) -> Result<(), Error> {
        let client = reqwest::Client::new();
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
