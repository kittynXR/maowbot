use async_trait::async_trait;
use crate::Error;

pub mod manager;
pub mod user_manager;
pub(crate) mod callback_server;

pub use manager::AuthManager;
pub use user_manager::{UserManager, DefaultUserManager};

#[derive(Debug, Clone)]
pub enum AuthenticationPrompt {
    Browser { url: String },
    Code { message: String },
    ApiKey { message: String },
    MultipleKeys { fields: Vec<String>, messages: Vec<String> },
    TwoFactor { message: String },
    None,
}

#[derive(Debug)]
pub enum AuthenticationResponse {
    Code(String),
    ApiKey(String),
    MultipleKeys(std::collections::HashMap<String, String>),
    TwoFactor(String),
    None,
}

#[async_trait]
pub trait AuthenticationHandler: Send + Sync {
    async fn handle_prompt(&self, prompt: AuthenticationPrompt) -> Result<AuthenticationResponse, Error>;
}

#[derive(Default)]
pub struct StubAuthHandler;

#[async_trait]
impl AuthenticationHandler for StubAuthHandler {
    async fn handle_prompt(&self, _prompt: AuthenticationPrompt) -> Result<AuthenticationResponse, Error> {
        // Always just return "None" or something minimal
        Ok(AuthenticationResponse::None)
    }
}

/// Each platform's authenticator will implement this.
#[async_trait]
pub trait PlatformAuthenticator: Send {
    async fn initialize(&mut self) -> Result<(), Error>;
    async fn start_authentication(&mut self) -> Result<AuthenticationPrompt, Error>;
    async fn complete_authentication(
        &mut self,
        response: AuthenticationResponse
    ) -> Result<crate::models::PlatformCredential, Error>;
    async fn refresh(&mut self, credential: &crate::models::PlatformCredential)
                     -> Result<crate::models::PlatformCredential, Error>;
    async fn validate(&self, credential: &crate::models::PlatformCredential) -> Result<bool, Error>;
    async fn revoke(&mut self, credential: &crate::models::PlatformCredential) -> Result<(), Error>;

    fn set_is_bot(&mut self, _val: bool) {
        // default is no-op, so that other authenticators won't break
    }
}