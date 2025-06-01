use async_trait::async_trait;
use chrono::Utc;
use serde_json::json;
use uuid::Uuid;

use crate::Error;
use maowbot_common::models::auth::{AuthenticationPrompt, AuthenticationResponse};
use maowbot_common::models::platform::{Platform, PlatformCredential};
use maowbot_common::models::credential::CredentialType;

use maowbot_common::traits::auth_traits::PlatformAuthenticator;

pub struct DiscordAuthenticator {
    client_id: Option<String>,
    client_secret: Option<String>,
    bot_token: Option<String>,
    is_bot: bool,
    is_broadcaster: bool,
    is_teammate: bool,
}

impl DiscordAuthenticator {
    /// Updated constructor: receives optional client_id and client_secret.
    pub fn new(
        client_id: Option<String>,
        client_secret: Option<String>,
    ) -> Self {
        Self {
            client_id,
            client_secret,
            bot_token: None,
            is_broadcaster: false,
            is_teammate: false,
            is_bot: false,
        }
    }

    /// Helper that, given a valid Bot token, calls Discord’s `GET /users/@me`
    /// and returns (user_id, username), or an Error if something fails.
    async fn fetch_discord_bot_info(&self, token: &str) -> Result<(String, String), Error> {
        use reqwest::Client;
        #[derive(serde::Deserialize)]
        struct DiscordUser {
            id: String,
            username: String,
            discriminator: String,
        }
        let client = Client::new();
        let resp = client
            .get("https://discord.com/api/v10/users/@me")
            .header("Authorization", format!("Bot {}", token))
            .send()
            .await
            .map_err(|e| Error::Auth(format!("Discord: error calling /users/@me => {e}")))?;

        if !resp.status().is_success() {
            return Err(Error::Auth(format!(
                "Discord: /users/@me returned HTTP {}",
                resp.status()
            )));
        }

        let user_obj = resp
            .json::<DiscordUser>()
            .await
            .map_err(|e| Error::Auth(format!("Discord: could not parse JSON => {e}")))?;

        Ok((user_obj.id, user_obj.username))
    }
}

#[async_trait]
impl PlatformAuthenticator for DiscordAuthenticator {
    async fn initialize(&mut self) -> Result<(), Error> {
        // No initialization needed for Discord Bot flow
        Ok(())
    }

    async fn start_authentication(&mut self) -> Result<AuthenticationPrompt, Error> {
        // We expect the user to enter multiple keys:
        //   - bot_token
        //   - bot_user_id (override or confirm from /users/@me)
        //   - bot_username (override or confirm from /users/@me)
        //   - bot_app_id (the new field that goes into refresh_token)
        Ok(AuthenticationPrompt::MultipleKeys {
            fields: vec![
                "bot_token".into(),
                "bot_user_id".into(),
                "bot_username".into(),
                "bot_app_id".into(),  // <--- NEW
            ],
            messages: vec![
                "Enter your Discord Bot Token".into(),
                "Enter your Discord Bot User ID".into(),
                "Enter your Discord Bot Username".into(),
                "Enter your Discord Application ID (App ID)".into(),
            ],
        })
    }

    async fn complete_authentication(
        &mut self,
        response: AuthenticationResponse,
    ) -> Result<PlatformCredential, Error> {
        match response {
            AuthenticationResponse::MultipleKeys(keys) => {
                // 1) Always expect a "bot_token"
                let bot_token = match keys.get("bot_token") {
                    Some(t) if !t.is_empty() => t.clone(),
                    _ => return Err(Error::Auth("Discord: 'bot_token' is required".into())),
                };
                self.bot_token = Some(bot_token.clone());

                // 2) Try calling /users/@me to confirm user ID, username
                //    (the user might override these anyway below).
                let (fetched_id, fetched_name) = match self.fetch_discord_bot_info(&bot_token).await {
                    Ok(res) => res,
                    Err(e) => {
                        // If we fail to fetch from Discord, we still proceed with user input,
                        // but warn them. We'll let them fill the fields manually.
                        eprintln!(
                            "(Warning) Could not fetch Discord bot info => {e}. \
                             Using user-provided fields if any..."
                        );
                        ("".to_string(), "".to_string())
                    }
                };

                let user_id = match keys.get("bot_user_id") {
                    Some(id) if !id.is_empty() => id.clone(),
                    _ => fetched_id,
                };
                if user_id.is_empty() {
                    return Err(Error::Auth(
                        "Discord: Bot user ID is required (none provided)".into(),
                    ));
                }

                let user_name = match keys.get("bot_username") {
                    Some(n) if !n.is_empty() => n.clone(),
                    _ => fetched_name,
                };
                if user_name.is_empty() {
                    return Err(Error::Auth(
                        "Discord: Bot username is required (none provided)".into(),
                    ));
                }

                // 3) Grab the "bot_app_id" from keys and store it in `refresh_token`.
                //    This is new: we place the app_id in the refresh_token field.
                let app_id = keys.get("bot_app_id").map(|s| s.clone());

                // 4) Build the final credential object
                Ok(PlatformCredential {
                    credential_id: Uuid::new_v4(),
                    platform: Platform::Discord,
                    credential_type: CredentialType::BearerToken,
                    user_id: Uuid::new_v4(), // gets overwritten by AuthManager
                    primary_token: bot_token,
                    refresh_token: app_id, // <--- store App ID here
                    additional_data: Some(json!({
                        "client_id": self.client_id,
                        "client_secret": self.client_secret
                    })),
                    expires_at: None,
                    created_at: Utc::now(),
                    updated_at: Utc::now(),
                    is_broadcaster: self.is_broadcaster,
                    is_teammate: self.is_teammate,
                    is_bot: self.is_bot,
                    platform_id: Some(user_id),
                    user_name,
                })
            }
            _ => Err(Error::Auth(
                "Invalid authentication response for Discord (expected multiple keys)".into(),
            )),
        }
    }

    async fn refresh(&mut self, credential: &PlatformCredential) -> Result<PlatformCredential, Error> {
        // Discord bot tokens do not expire, so we simply return the existing credential.
        Ok(credential.clone())
    }

    async fn validate(&self, credential: &PlatformCredential) -> Result<bool, Error> {
        use reqwest::Client;
        let client = Client::new();
        let response = client
            .get("https://discord.com/api/v10/users/@me")
            .header("Authorization", format!("Bot {}", credential.primary_token))
            .send()
            .await?;
        Ok(response.status().is_success())
    }

    async fn revoke(&mut self, _credential: &PlatformCredential) -> Result<(), Error> {
        // Bot tokens cannot be revoked programmatically at present.
        Ok(())
    }

    fn set_is_broadcaster(&mut self, val: bool) {
        self.is_broadcaster = val;
    }

    fn set_is_teammate(&mut self, val: bool) {
        self.is_teammate = val;
    }

    fn set_is_bot(&mut self, val: bool) {
        self.is_bot = val;
    }
}
