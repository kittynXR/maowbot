// File: src/platforms/twitch/auth.rs

use async_trait::async_trait;
use oauth2::{
    AuthUrl, ClientId, ClientSecret, RedirectUrl, TokenUrl,
    basic::BasicClient, AuthorizationCode, TokenResponse,
    CsrfToken, Scope,
};
use chrono::Utc;
use serde_json::json;
use uuid::Uuid;

use crate::Error;
use crate::models::{Platform, PlatformCredential, CredentialType};
use crate::auth::{AuthenticationPrompt, AuthenticationResponse, PlatformAuthenticator};

pub struct TwitchAuthenticator {
    client_id: String,
    client_secret: String,
    oauth_client: BasicClient,
    state: CsrfToken,
}

impl TwitchAuthenticator {
    pub fn new(client_id: String, client_secret: String, redirect_uri: String) -> Self {
        let oauth_client = BasicClient::new(
            ClientId::new(client_id.clone()),
            Some(ClientSecret::new(client_secret.clone())),
            AuthUrl::new("https://id.twitch.tv/oauth2/authorize".to_string()).unwrap(),
            Some(TokenUrl::new("https://id.twitch.tv/oauth2/token".to_string()).unwrap())
        )
            .set_redirect_uri(RedirectUrl::new(redirect_uri).unwrap());

        Self {
            client_id,
            client_secret,
            oauth_client,
            state: CsrfToken::new_random(),
        }
    }
}

#[async_trait]
impl PlatformAuthenticator for TwitchAuthenticator {
    async fn initialize(&mut self) -> Result<(), Error> {
        Ok(())
    }

    async fn start_authentication(&mut self) -> Result<AuthenticationPrompt, Error> {
        let (auth_url, _csrf_state) = self.oauth_client
            .authorize_url(|| self.state.clone())
            .add_scope(Scope::new("chat:read".to_string()))
            .add_scope(Scope::new("chat:edit".to_string()))
            .url();

        Ok(AuthenticationPrompt::Browser {
            url: auth_url.to_string()
        })
    }

    async fn complete_authentication(
        &mut self,
        response: AuthenticationResponse,
    ) -> Result<PlatformCredential, Error> {
        let code = match response {
            AuthenticationResponse::Code(code) => code,
            _ => return Err(Error::Auth("Invalid authentication response".into())),
        };

        let token = self.oauth_client
            .exchange_code(AuthorizationCode::new(code))
            .request_async(oauth2::reqwest::async_http_client)
            .await
            .map_err(|e| Error::Auth(e.to_string()))?;

        Ok(PlatformCredential {
            credential_id: Uuid::new_v4().to_string(),
            platform: Platform::Twitch,
            credential_type: CredentialType::OAuth2,
            user_id: String::new(),
            primary_token: token.access_token().secret().clone(),
            refresh_token: token.refresh_token().map(|t| t.secret().clone()),
            additional_data: Some(json!({
                "scope": token.scopes()
                    .map(|scopes| scopes.iter()
                        .map(|s| s.to_string())
                        .collect::<Vec<String>>())
                    .unwrap_or_default()
            })),
            expires_at: token.expires_in().map(|d|
                Utc::now().naive_utc() + chrono::Duration::from_std(d).unwrap()
            ),
            created_at: Utc::now().naive_utc(),
            updated_at: Utc::now().naive_utc(),
        })
    }

    async fn refresh(&mut self, credential: &PlatformCredential) -> Result<PlatformCredential, Error> {
        let refresh_token = credential.refresh_token.as_ref()
            .ok_or_else(|| Error::Auth("No refresh token available".into()))?;

        let token = self.oauth_client
            .exchange_refresh_token(&oauth2::RefreshToken::new(refresh_token.clone()))
            .request_async(oauth2::reqwest::async_http_client)
            .await
            .map_err(|e| Error::Auth(e.to_string()))?;

        Ok(PlatformCredential {
            credential_id: credential.credential_id.clone(),
            platform: Platform::Twitch,
            credential_type: CredentialType::OAuth2,
            user_id: credential.user_id.clone(),
            primary_token: token.access_token().secret().clone(),
            refresh_token: token.refresh_token().map(|t| t.secret().clone()),
            additional_data: Some(json!({
                "scope": token.scopes()
                    .map(|scopes| scopes.iter()
                        .map(|s| s.to_string())
                        .collect::<Vec<String>>())
                    .unwrap_or_default()
            })),
            expires_at: token.expires_in().map(|d|
                Utc::now().naive_utc() + chrono::Duration::from_std(d).unwrap()
            ),
            created_at: credential.created_at,
            updated_at: Utc::now().naive_utc(),
        })
    }

    async fn validate(&self, credential: &PlatformCredential) -> Result<bool, Error> {
        let client = reqwest::Client::new();
        let response = client.get("https://api.twitch.tv/helix/users")
            .header("Authorization", format!("Bearer {}", credential.primary_token))
            .header("Client-Id", &self.client_id)
            .send()
            .await?;

        Ok(response.status().is_success())
    }

    async fn revoke(&mut self, credential: &PlatformCredential) -> Result<(), Error> {
        let client = reqwest::Client::new();
        let response = client.post("https://id.twitch.tv/oauth2/revoke")
            .query(&[
                ("client_id", &self.client_id),
                ("token", &credential.primary_token),
            ])
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(Error::Auth("Failed to revoke token".into()));
        }

        Ok(())
    }
}
