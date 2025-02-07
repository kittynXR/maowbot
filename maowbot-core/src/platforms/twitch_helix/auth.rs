use std::sync::Arc;
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
    AuthUrl, AuthorizationCode, ClientId, ClientSecret, TokenUrl,
    RefreshToken, Scope, TokenResponse, PkceCodeChallenge, PkceCodeVerifier,
};
use serde_json::json;
use uuid::Uuid;
use tracing::{info, error};

use crate::auth::callback_server;
use crate::Error;
use crate::auth::{AuthenticationPrompt, AuthenticationResponse, PlatformAuthenticator};
use crate::models::{CredentialType, Platform, PlatformCredential};
use crate::repositories::postgres::bot_config::BotConfigRepository;

pub struct TwitchAuthenticator {
    pub client_id: String,
    pub client_secret: Option<String>,

    /// The OAuth2 Client
    pub oauth_client: OAuthClient,

    /// Reference to whichever repo can fetch/set `callback_port`.
    pub bot_config_repo: Arc<dyn BotConfigRepository + Send + Sync>,

    pub pkce_verifier: Option<PkceCodeVerifier>,
    pub is_bot: bool,
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

impl TwitchAuthenticator {
    pub fn new(
        client_id: String,
        client_secret: Option<String>,
        bot_config_repo: Arc<dyn BotConfigRepository + Send + Sync>,
    ) -> Self {
        let mut oauth_client = Client::new(ClientId::new(client_id.clone()))
            .set_auth_uri(AuthUrl::new("https://id.twitch.tv/oauth2/authorize".to_string()).unwrap())
            .set_token_uri(TokenUrl::new("https://id.twitch.tv/oauth2/token".to_string()).unwrap());

        if let Some(sec) = &client_secret {
            oauth_client = oauth_client.set_client_secret(ClientSecret::new(sec.clone()));
        }

        Self {
            client_id,
            client_secret,
            oauth_client,
            bot_config_repo,
            pkce_verifier: None,
            is_bot: false,
        }
    }
}

#[async_trait]
impl PlatformAuthenticator for TwitchAuthenticator {
    async fn initialize(&mut self) -> Result<(), Error> {
        Ok(())
    }

    async fn start_authentication(&mut self) -> Result<AuthenticationPrompt, Error> {
        // Now we can fetch the callback port from self.bot_config_repo
        let port = callback_server::get_or_fix_callback_port(&*self.bot_config_repo).await?;
        let redirect_uri = format!("http://localhost:{}/callback", port);
        self.oauth_client = self.oauth_client.clone().set_redirect_uri(
            oauth2::RedirectUrl::new(redirect_uri)
                .map_err(|e| Error::Auth(e.to_string()))?
        );

        // Generate PKCE
        let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();
        self.pkce_verifier = Some(pkce_verifier);

        let (auth_url, _csrf_state) = self
            .oauth_client
            .authorize_url(|| oauth2::CsrfToken::new_random())
            .set_pkce_challenge(pkce_challenge)
            .add_scope(Scope::new("chat:read".into()))
            .add_scope(Scope::new("chat:edit".into()))
            .url();

        info!("[TwitchAuthenticator] Created auth URL => {}", auth_url);
        Ok(AuthenticationPrompt::Browser {
            url: auth_url.to_string(),
        })
    }

    async fn complete_authentication(
        &mut self,
        response: AuthenticationResponse
    ) -> Result<PlatformCredential, Error> {
        let code_str = match response {
            AuthenticationResponse::Code(c) => c,
            _ => return Err(Error::Auth("Expected code in TwitchAuthenticator::complete_authentication".into())),
        };

        let http_client = reqwest::Client::new();
        let pkce_verifier = self.pkce_verifier.take()
            .ok_or_else(|| Error::Auth("Missing PKCE verifier".into()))?;

        let token_res = self
            .oauth_client
            .exchange_code(AuthorizationCode::new(code_str))
            .set_pkce_verifier(pkce_verifier)
            .request_async(&http_client)
            .await
            .map_err(|e| Error::Auth(e.to_string()))?;

        let cred = PlatformCredential {
            credential_id: Uuid::new_v4().to_string(),
            platform: Platform::Twitch,
            credential_type: CredentialType::OAuth2,
            user_id: String::new(),
            primary_token: token_res.access_token().secret().to_owned(),
            refresh_token: token_res.refresh_token().map(|x| x.secret().to_owned()),
            additional_data: Some(json!({
                "scope": token_res.scopes().map(|sc| sc.iter().map(|s| s.to_string()).collect::<Vec<_>>()).unwrap_or_default()
            })),
            expires_at: token_res.expires_in().map(|d| Utc::now() + chrono::Duration::from_std(d).unwrap()),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            is_bot: self.is_bot,
        };
        Ok(cred)
    }

    async fn refresh(&mut self, credential: &PlatformCredential) -> Result<PlatformCredential, Error> {
        // if we have a refresh_token
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

    fn set_is_bot(&mut self, val: bool) {
        self.is_bot = val;
    }
}