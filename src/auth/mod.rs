// src/auth/mod.rs
use async_trait::async_trait;
use crate::Error;
use crate::models::{Platform, PlatformCredential};
use std::collections::HashMap;

pub mod platforms;
pub mod manager;

pub use manager::AuthManager;

pub use self::platforms::{
    TwitchAuthenticator,
    DiscordAuthenticator,
    VRChatAuthenticator,
};

#[derive(Debug, Clone)]
pub enum AuthenticationPrompt {
    Browser { url: String },
    Code { message: String },
    ApiKey { message: String },
    MultipleKeys { fields: Vec<String>, messages: Vec<String> },
    TwoFactor { message: String },
    None
}

#[derive(Debug)]
pub enum AuthenticationResponse {
    Code(String),
    ApiKey(String),
    MultipleKeys(HashMap<String, String>),
    TwoFactor(String),
    None
}

#[async_trait]
pub trait AuthenticationHandler {
    async fn handle_prompt(&self, prompt: AuthenticationPrompt) -> Result<AuthenticationResponse, Error>;
}

#[async_trait]
pub trait PlatformAuthenticator: Send {
    async fn initialize(&mut self) -> Result<(), Error>;

    async fn start_authentication(&mut self) -> Result<AuthenticationPrompt, Error>;

    async fn complete_authentication(
        &mut self,
        response: AuthenticationResponse
    ) -> Result<PlatformCredential, Error>;

    async fn refresh(&mut self, credential: &PlatformCredential) -> Result<PlatformCredential, Error>;

    async fn validate(&self, credential: &PlatformCredential) -> Result<bool, Error>;

    async fn revoke(&mut self, credential: &PlatformCredential) -> Result<(), Error>;
}