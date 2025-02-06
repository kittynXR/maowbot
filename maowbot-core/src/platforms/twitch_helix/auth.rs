// File: maowbot-core/src/platforms/twitch_helix/auth.rs

use async_trait::async_trait;
use chrono::Utc;
use oauth2::{
    basic::{
        BasicErrorResponse,
        BasicRevocationErrorResponse,
        BasicTokenIntrospectionResponse,
        BasicTokenResponse,
    },
    Client, EndpointNotSet, EndpointSet, StandardRevocableToken,
    AuthUrl, AuthorizationCode, ClientId, ClientSecret, CsrfToken,
    RedirectUrl, RefreshToken, Scope, TokenResponse, TokenUrl,
    PkceCodeChallenge, PkceCodeVerifier,
};
use serde_json::json;
use uuid::Uuid;
use tracing::{info, error};

use crate::auth::callback_server;
use crate::Error;
use crate::auth::{AuthenticationPrompt, AuthenticationResponse, PlatformAuthenticator};
use crate::models::{CredentialType, Platform, PlatformCredential};
use crate::repositories::postgres::app_config::AppConfigRepository;

/// The "TwitchAuthenticator" struct now keeps pkce_challenge/pkce_verifier, plus a user-chosen `is_bot`.
pub struct TwitchAuthenticator<A: AppConfigRepository + Send + Sync> {
    client_id: String,
    client_secret: String,
    oauth_client: OAuthClient,
    app_config_repo: A,
    pkce_verifier: Option<PkceCodeVerifier>,
    is_bot: bool,
}

type OAuthClient = Client<
    BasicErrorResponse,
    BasicTokenResponse,
    BasicTokenIntrospectionResponse,
    StandardRevocableToken,
    BasicRevocationErrorResponse,
    EndpointSet,
    EndpointNotSet,
    EndpointNotSet,
    EndpointNotSet,
    EndpointSet
>;

impl<A> TwitchAuthenticator<A>
where
    A: AppConfigRepository + Send + Sync + 'static,
{
    pub fn new(client_id: String, client_secret: String, app_config_repo: A) -> Self {
        // We do not set redirect URI yet; we will do it dynamically once we have a valid port
        let oauth_client = Client::new(ClientId::new(client_id.clone()))
            .set_client_secret(ClientSecret::new(client_secret.clone()))
            .set_auth_uri(AuthUrl::new("https://id.twitch.tv/oauth2/authorize".to_string()).unwrap())
            .set_token_uri(TokenUrl::new("https://id.twitch.tv/oauth2/token".to_string()).unwrap());

        Self {
            client_id,
            client_secret,
            oauth_client,
            app_config_repo,
            pkce_verifier: None,
            is_bot: false,
        }
    }

    pub fn set_is_bot_flag(&mut self, val: bool) {
        self.is_bot = val;
    }
}

#[async_trait]
impl<A> PlatformAuthenticator for TwitchAuthenticator<A>
where
    A: AppConfigRepository + Send + Sync + 'static,
{
    /// Called once before `start_authentication`, good for reading config or verifying things.
    async fn initialize(&mut self) -> Result<(), Error> {
        Ok(())
    }

    /// In a real PKCE flow, we build the authorize URL with a code_challenge, spawn the local callback, etc.
    async fn start_authentication(&mut self) -> Result<AuthenticationPrompt, Error> {
        // 1) Determine the callback port from DB or fix it if conflict
        let port = callback_server::get_or_fix_callback_port(&self.app_config_repo).await?;
        let redirect_uri = format!("http://localhost:{}/callback", port);
        self.oauth_client = self.oauth_client.clone().set_redirect_uri(
            RedirectUrl::new(redirect_uri.clone()).map_err(|e| Error::Auth(e.to_string()))?
        );

        // 2) Generate PKCE code challenge
        let (pkce_challenge, pkce_verifier) = oauth2::PkceCodeChallenge::new_random_sha256();
        self.pkce_verifier = Some(pkce_verifier);

        // 3) Build full Twitch auth URL
        //    e.g. requesting scopes: "user:read:email", "chat:read", "chat:edit", etc.
        let state = CsrfToken::new_random();
        let (auth_url, _csrf_state) = self
            .oauth_client
            .authorize_url(|| state)
            .set_pkce_challenge(pkce_challenge)
            .add_scope(Scope::new("chat:read".to_string()))
            .add_scope(Scope::new("chat:edit".to_string()))
            .url();

        // 4) Return a prompt that instructs user to open a browser to that URL
        info!("[TwitchAuthenticator] Created auth URL => {}", auth_url);
        Ok(AuthenticationPrompt::Browser {
            url: auth_url.to_string(),
        })
    }

    /// We'll receive the code from the TUI or automated flow (which should have come from the callback server).
    async fn complete_authentication(
        &mut self,
        response: crate::auth::AuthenticationResponse,
    ) -> Result<PlatformCredential, Error> {
        // The TUI or the top-level manager should have stored the code from the callback server in AuthenticationResponse::Code
        let code_str = match response {
            AuthenticationResponse::Code(c) => c,
            _ => {
                return Err(Error::Auth(
                    "Expected a code in complete_authentication for Twitch PKCE".into()
                ));
            }
        };

        let http_client = reqwest::Client::new();

        let pkce_verifier = match self.pkce_verifier.take() {
            Some(v) => v,
            None => {
                return Err(Error::Auth("PKCE verifier was missing".into()));
            }
        };

        // Now exchange code for token
        let token_res = self
            .oauth_client
            .exchange_code(AuthorizationCode::new(code_str))
            .set_pkce_verifier(pkce_verifier)
            .request_async(&http_client)
            .await
            .map_err(|e| Error::Auth(e.to_string()))?;

        // Build final credential
        let credential = PlatformCredential {
            credential_id: Uuid::new_v4().to_string(),
            platform: Platform::Twitch,
            credential_type: CredentialType::OAuth2,
            user_id: String::new(), // The manager might associate a user_id after the fact, or keep it blank
            primary_token: token_res.access_token().secret().to_owned(),
            refresh_token: token_res.refresh_token().map(|r| r.secret().to_owned()),
            additional_data: Some(json!({
                "scope": token_res
                    .scopes()
                    .map(|sc| sc.iter().map(|s| s.to_string()).collect::<Vec<_>>())
                    .unwrap_or_default()
            })),
            expires_at: token_res.expires_in().map(|dur| {
                Utc::now() + chrono::Duration::from_std(dur).unwrap()
            }),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            is_bot: self.is_bot,
        };
        Ok(credential)
    }

    /// Refresh token if present
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

        let new_cred = PlatformCredential {
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
                Utc::now() + chrono::Duration::from_std(dur).unwrap()
            }),
            created_at: credential.created_at,
            updated_at: Utc::now(),
            is_bot: credential.is_bot,
        };
        Ok(new_cred)
    }

    /// Validate an existing credential
    async fn validate(&self, credential: &PlatformCredential) -> Result<bool, Error> {
        let client = reqwest::Client::new();
        let response = client
            .get("https://api.twitch.tv/helix/users")
            .header("Authorization", format!("Bearer {}", credential.primary_token))
            .header("Client-Id", &self.client_id)
            .send()
            .await?;

        Ok(response.status().is_success())
    }

    /// Revoke
    async fn revoke(&mut self, credential: &PlatformCredential) -> Result<(), Error> {
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
            error!("Failed to revoke token: {:?}", resp.text().await);
            return Err(Error::Auth("Failed to revoke token".into()));
        }
        Ok(())
    }
}

impl<A> TwitchAuthenticator<A>
where
    A: AppConfigRepository + Send + Sync + 'static,
{
    /// So the manager can set whether we store `is_bot = true`.
    pub fn set_is_bot(&mut self, is_bot: bool) {
        self.is_bot = is_bot;
    }
}