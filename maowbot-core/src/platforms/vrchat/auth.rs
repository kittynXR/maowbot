use async_trait::async_trait;
use chrono::{Duration, Utc};
use http::header::USER_AGENT;
use http::{HeaderMap, HeaderValue};
use serde_json::json;
use uuid::Uuid;

use crate::Error;
use crate::auth::{AuthenticationPrompt, AuthenticationResponse, PlatformAuthenticator};
use crate::models::{CredentialType, Platform, PlatformCredential};

const VRCHAT_UA: &str = "MaowBot/1.0 cat@kittyn.cat";

/// For 2FA detection we parse the JSON from the VRChat response. If "requiresTwoFactorAuth": ["totp"] or ["emailOtp"], we proceed to 2FA.
#[derive(Debug, serde::Deserialize)]
struct LoginResponse {
    #[serde(default)]
    requires_two_factor_auth: Option<Vec<String>>,
    // ... other fields ...
}

/// A minimal VRChat authenticator that uses raw `reqwest` to do:
///   - GET /auth/user with Basic Auth for initial login
///   - If 2FA is required, POST code to the relevant endpoint
///   - Each time, parse "Set-Cookie: auth=..." from the response header
pub struct VRChatAuthenticator {
    username: Option<String>,
    password: Option<String>,
    two_factor_code: Option<String>,
    is_bot: bool,
    two_factor_method: Option<String>,
    session_cookie: Option<String>,
}

impl VRChatAuthenticator {
    pub fn new() -> Self {
        Self {
            username: None,
            password: None,
            two_factor_code: None,
            is_bot: false,
            two_factor_method: None,
            session_cookie: None,
        }
    }

    async fn attempt_login(&mut self) -> Result<(), Error> {
        let username = self.username.as_ref()
            .ok_or_else(|| Error::Auth("VRChat: missing username".into()))?;
        let password = self.password.as_ref()
            .ok_or_else(|| Error::Auth("VRChat: missing password".into()))?;

        let mut default_headers = HeaderMap::new();
        default_headers.insert(USER_AGENT, HeaderValue::from_str(VRCHAT_UA)
            .map_err(|e| Error::Auth(format!("Invalid UA string: {e}")))?
        );

        // 2) Create a reqwest client with these default headers
        let client = reqwest::ClientBuilder::new()
            .default_headers(default_headers)
            .build()
            .map_err(|e| Error::Auth(format!("reqwest build error: {e}")))?;

        // 2) GET /auth/user with basic_auth.
        let resp = client
            .get("https://api.vrchat.cloud/api/1/auth/user")
            .basic_auth(username, Some(password))
            .send()
            .await
            .map_err(|e| Error::Auth(format!("VRChat login request error: {e}")))?;

        // 3) Immediately capture the status and headers.
        let status = resp.status();
        let headers: HeaderMap = resp.headers().clone();

        // 4) If status is not success, read the body text and return error.
        if !status.is_success() {
            let body_text = resp.text().await.unwrap_or_default();
            return Err(Error::Auth(format!("VRChat login error: HTTP {status}, {body_text}")));
        }

        // 5) Now parse the JSON body. This consumes resp.
        let json_val: serde_json::Value = resp
            .json::<serde_json::Value>()
            .await
            .map_err(|e| Error::Auth(format!("Parsing VRChat login JSON => {e}")))?;

        // 6) Check if the JSON indicates that 2FA is required.
        if let Some(arr) = json_val["requiresTwoFactorAuth"].as_array() {
            if !arr.is_empty() {
                self.two_factor_method = Some(arr[0].as_str().unwrap_or("totp").to_string());
                return Err(Error::Auth("2FA required".into()));
            }
        }

        // 7) Parse the "Set-Cookie" headers for an auth cookie.
        let set_cookie_headers = headers.get_all("set-cookie");
        let auth_cookie = parse_auth_cookie_from_headers(set_cookie_headers)?;
        self.session_cookie = Some(auth_cookie);

        Ok(())
    }

    /// Helper: extract the first "auth=..." value from the given headers.
    fn parse_auth_cookie_from_headers(
        set_cookie_headers: reqwest::header::GetAll<reqwest::header::HeaderValue>
    ) -> Result<String, Error> {
        for value in set_cookie_headers {
            if let Ok(val_str) = value.to_str() {
                if val_str.starts_with("auth=") {
                    // For example: "auth=abc123; Path=/; HttpOnly; Secure"
                    if let Some(semicolon_pos) = val_str.find(';') {
                        return Ok(val_str[..semicolon_pos].to_string());
                    } else {
                        return Ok(val_str.to_string());
                    }
                }
            }
        }
        Err(Error::Auth("Could not find 'auth=' cookie in Set-Cookie".into()))
    }

    async fn attempt_2fa(&mut self) -> Result<(), Error> {
        let code = self.two_factor_code.as_ref()
            .ok_or_else(|| Error::Auth("No 2FA code provided".into()))?;
        let method = self.two_factor_method.clone()
            .unwrap_or_else(|| "totp".into());

        let username = self.username.as_ref()
            .ok_or_else(|| Error::Auth("VRChat: missing username".into()))?;
        let password = self.password.as_ref()
            .ok_or_else(|| Error::Auth("VRChat: missing password".into()))?;

        let mut default_headers = HeaderMap::new();
        default_headers.insert(USER_AGENT, HeaderValue::from_str(VRCHAT_UA)
            .map_err(|e| Error::Auth(format!("Invalid UA string: {e}")))?
        );

        // 2) Create a reqwest client with these default headers
        let client = reqwest::ClientBuilder::new()
            .default_headers(default_headers)
            .build()
            .map_err(|e| Error::Auth(format!("reqwest build error: {e}")))?;

        // VRChat 2FA endpoint might differ by method. Example:
        let twofa_url = if method == "emailOtp" {
            "https://api.vrchat.cloud/api/1/auth/twofactorauth/emailotp/verify"
        } else {
            "https://api.vrchat.cloud/api/1/auth/twofactorauth/totp/verify"
        };

        let body_json = serde_json::json!({ "code": code });
        let resp = client
            .post(twofa_url)
            .basic_auth(username, Some(password))
            .json(&body_json)
            .send()
            .await
            .map_err(|e| Error::Auth(format!("VRChat 2FA request error: {e}")))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body_text = resp.text().await.unwrap_or_default();
            return Err(Error::Auth(format!("VRChat 2FA error: HTTP {status}, {body_text}")));
        }

        // On success, we expect a new `auth=...` cookie in `Set-Cookie`
        let set_cookie_headers = resp.headers().get_all("set-cookie");
        let auth_cookie = parse_auth_cookie_from_headers(set_cookie_headers)?;
        self.session_cookie = Some(auth_cookie);

        Ok(())
    }

    fn build_credential(&self) -> Result<PlatformCredential, Error> {
        let cookie = self.session_cookie.as_ref()
            .ok_or_else(|| Error::Auth("No VRChat session cookie stored.".into()))?
            .clone();
        let now = Utc::now();
        let expires_at = Some(now + Duration::days(30));
        let user_name = self
            .username
            .clone()
            .unwrap_or_else(|| "unknown".into());

        Ok(PlatformCredential {
            credential_id: Uuid::new_v4(),
            platform: Platform::VRChat,
            platform_id: Some(user_name.clone()),
            credential_type: CredentialType::Interactive2FA,
            user_id: Uuid::new_v4(),
            user_name,
            primary_token: cookie, // "auth=AbCdEf..."
            refresh_token: None,
            additional_data: Some(json!({ "two_factor_method": self.two_factor_method })),
            expires_at,
            created_at: now,
            updated_at: now,
            is_bot: self.is_bot,
        })
    }
}

#[async_trait]
impl PlatformAuthenticator for VRChatAuthenticator {
    async fn initialize(&mut self) -> Result<(), Error> {
        self.username = None;
        self.password = None;
        self.two_factor_code = None;
        self.is_bot = false;
        self.two_factor_method = None;
        self.session_cookie = None;
        Ok(())
    }

    async fn start_authentication(&mut self) -> Result<AuthenticationPrompt, Error> {
        // Step 1 => we want username & password
        Ok(AuthenticationPrompt::MultipleKeys {
            fields: vec!["username".into(), "password".into()],
            messages: vec![
                "Enter your VRChat username:".into(),
                "Enter your VRChat password:".into(),
            ],
        })
    }

    async fn complete_authentication(
        &mut self,
        response: AuthenticationResponse
    ) -> Result<PlatformCredential, Error> {
        match response {
            AuthenticationResponse::MultipleKeys(keys) => {
                self.username = keys.get("username").cloned();
                self.password = keys.get("password").cloned();

                let attempt = self.attempt_login().await;
                match attempt {
                    Ok(_) => {
                        // no 2FA needed
                        self.build_credential()
                    }
                    Err(e) => {
                        let msg = format!("{e}");
                        if msg.contains("2FA required") {
                            Err(Error::Auth("__2FA_PROMPT__".into()))
                        } else {
                            Err(e)
                        }
                    }
                }
            }
            AuthenticationResponse::TwoFactor(code) => {
                self.two_factor_code = Some(code);
                self.attempt_2fa().await?;
                self.build_credential()
            }
            _ => Err(Error::Auth("VRChat: unexpected flow response".into())),
        }
    }

    async fn refresh(&mut self, _credential: &PlatformCredential) -> Result<PlatformCredential, Error> {
        Err(Error::Auth("VRChat does not offer a refresh flow; re-login required.".into()))
    }

    async fn validate(&self, credential: &PlatformCredential) -> Result<bool, Error> {
        // Minimal check: if it starts with "auth=", assume valid.
        // More robust: do GET /auth/user with that cookie, see if 200.
        Ok(credential.primary_token.starts_with("auth="))
    }

    async fn revoke(&mut self, _credential: &PlatformCredential) -> Result<(), Error> {
        // VRChat: Typically session is ended by letting the cookie expire or manually from website.
        Ok(())
    }

    fn set_is_bot(&mut self, val: bool) {
        self.is_bot = val;
    }
}

/// Helper to parse the `"auth=..."` cookie from the "Set-Cookie" headers
pub(crate) fn parse_auth_cookie_from_headers(
    set_cookie_headers: reqwest::header::GetAll<reqwest::header::HeaderValue>
) -> Result<String, Error> {
    for value in set_cookie_headers {
        if let Ok(val_str) = value.to_str() {
            if val_str.starts_with("auth=") {
                // e.g. "auth=abc123; Path=/; HttpOnly; Secure"
                let semicolon_pos = val_str.find(';').unwrap_or(val_str.len());
                let cookie_sub = &val_str[..semicolon_pos];
                return Ok(cookie_sub.to_string()); // "auth=abc123"
            }
        }
    }
    Err(Error::Auth("Could not find 'auth=' cookie in Set-Cookie".into()))
}
