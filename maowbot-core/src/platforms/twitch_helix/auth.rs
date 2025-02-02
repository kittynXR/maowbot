// File: src/platforms/twitch_helix/auth.rs

use async_trait::async_trait;
use chrono::Utc;
use oauth2::{
    basic::{
        BasicErrorResponse,
        BasicRevocationErrorResponse,
        BasicTokenIntrospectionResponse,
        BasicTokenResponse,
    },
    // The core Client plus EndpointSet, etc.
    Client, EndpointNotSet, EndpointSet, StandardRevocableToken,
    AuthUrl, AuthorizationCode, ClientId, ClientSecret, CsrfToken, RedirectUrl,
    RefreshToken, Scope, TokenResponse, TokenUrl,
};
use serde_json::json;
use uuid::Uuid;

use crate::auth::{AuthenticationPrompt, AuthenticationResponse, PlatformAuthenticator};
use crate::models::{CredentialType, Platform, PlatformCredential};
use crate::Error;

// 1) Define an alias that has auth + token endpoints set (EndpointSet).
//    We don't need device/introspection/revocation endpoints set for this example,
//    so those remain EndpointNotSet.
type TwitchOAuthClient = Client<
    BasicErrorResponse,
    BasicTokenResponse,
    BasicTokenIntrospectionResponse,
    StandardRevocableToken,
    BasicRevocationErrorResponse,
    EndpointSet,     // HasAuthUrl
    EndpointNotSet,  // HasDeviceAuthUrl
    EndpointNotSet,  // HasIntrospectionUrl
    EndpointNotSet,  // HasRevocationUrl
    EndpointSet      // HasTokenUrl
>;

pub struct TwitchAuthenticator {
    client_id: String,
    client_secret: String,

    // 2) Store the final "fully configured" type here:
    oauth_client: TwitchOAuthClient,

    // We'll keep one CsrfToken around. Or you can generate a new one each time.
    state: CsrfToken,
}

impl TwitchAuthenticator {
    pub fn new(client_id: String, client_secret: String, redirect_uri: String) -> Self {
        // 3) Build the client with the required endpoints in one chain.
        //    The final result is no longer "BasicClient" but "TwitchOAuthClient".
        let oauth_client = Client::new(ClientId::new(client_id.clone()))
            .set_client_secret(ClientSecret::new(client_secret.clone()))
            .set_auth_uri(AuthUrl::new("https://id.twitch.tv/oauth2/authorize".to_string()).unwrap())
            .set_token_uri(TokenUrl::new("https://id.twitch.tv/oauth2/token".to_string()).unwrap())
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
        // If you have any special startup logic, do it here.
        Ok(())
    }

    async fn start_authentication(&mut self) -> Result<AuthenticationPrompt, Error> {
        let (auth_url, _csrf_state) = self
            .oauth_client
            .authorize_url(|| self.state.clone()) // or CsrfToken::new_random
            .add_scope(Scope::new("chat:read".to_string()))
            .add_scope(Scope::new("chat:edit".to_string()))
            .url();

        Ok(AuthenticationPrompt::Browser {
            url: auth_url.to_string(),
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

        let http_client = reqwest::Client::new();

        let token = self
            .oauth_client
            .exchange_code(AuthorizationCode::new(code))
            .request_async(&http_client)
            .await
            .map_err(|e| Error::Auth(e.to_string()))?;

        Ok(PlatformCredential {
            credential_id: Uuid::new_v4().to_string(),
            platform: Platform::Twitch,
            credential_type: CredentialType::OAuth2,
            user_id: String::new(),
            primary_token: token.access_token().secret().to_owned(),
            refresh_token: token.refresh_token().map(|r| r.secret().to_owned()),
            additional_data: Some(json!({
                "scope": token
                    .scopes()
                    .map(|sc| sc.iter().map(|s| s.to_string()).collect::<Vec<_>>())
                    .unwrap_or_default()
            })),
            expires_at: token.expires_in().map(|dur| {
                Utc::now().naive_utc() + chrono::Duration::from_std(dur).unwrap()
            }),
            created_at: Utc::now().naive_utc(),
            updated_at: Utc::now().naive_utc(),
        })
    }

    async fn refresh(&mut self, credential: &PlatformCredential) -> Result<PlatformCredential, Error> {
        let refresh_token = credential
            .refresh_token
            .as_ref()
            .ok_or_else(|| Error::Auth("No refresh token available".into()))?;

        let http_client = reqwest::Client::new();
        let token = self
            .oauth_client
            .exchange_refresh_token(&RefreshToken::new(refresh_token.clone()))
            .request_async(&http_client)
            .await
            .map_err(|e| Error::Auth(e.to_string()))?;

        Ok(PlatformCredential {
            credential_id: credential.credential_id.clone(),
            platform: Platform::Twitch,
            credential_type: CredentialType::OAuth2,
            user_id: credential.user_id.clone(),
            primary_token: token.access_token().secret().to_owned(),
            refresh_token: token.refresh_token().map(|r| r.secret().to_owned()),
            additional_data: Some(json!({
                "scope": token
                    .scopes()
                    .map(|sc| sc.iter().map(|s| s.to_string()).collect::<Vec<_>>())
                    .unwrap_or_default()
            })),
            expires_at: token.expires_in().map(|dur| {
                Utc::now().naive_utc() + chrono::Duration::from_std(dur).unwrap()
            }),
            created_at: credential.created_at,
            updated_at: Utc::now().naive_utc(),
        })
    }

    async fn validate(&self, credential: &PlatformCredential) -> Result<bool, Error> {
        // Just call Twitch's "validate user" endpoint directly via reqwest
        let client = reqwest::Client::new();
        let response = client
            .get("https://api.twitch.tv/helix/users")
            .header("Authorization", format!("Bearer {}", credential.primary_token))
            .header("Client-Id", &self.client_id)
            .send()
            .await?;

        Ok(response.status().is_success())
    }

    async fn revoke(&mut self, credential: &PlatformCredential) -> Result<(), Error> {
        // Twitch's revocation endpoint, done manually
        let client = reqwest::Client::new();
        let resp = client
            .post("https://id.twitch.tv/oauth2/revoke")
            .query(&[
                ("client_id", &self.client_id),
                ("token", &credential.primary_token),
            ])
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(Error::Auth("Failed to revoke token".into()));
        }
        Ok(())
    }
}
