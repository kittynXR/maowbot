// File: src/auth/mod.rs
use async_trait::async_trait;
use crate::Error;

pub mod manager;

// The central traits you introduced
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
pub trait AuthenticationHandler {
    async fn handle_prompt(&self, prompt: AuthenticationPrompt) -> Result<AuthenticationResponse, Error>;
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
}

pub use manager::AuthManager;
