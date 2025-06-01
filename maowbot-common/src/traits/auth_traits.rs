use async_trait::async_trait;
use uuid::Uuid;
pub use crate::models::auth::{AuthenticationPrompt, AuthenticationResponse, Platform};
use crate::error::Error;
use crate::models::platform::PlatformCredential;
use crate::models::user::User;
use crate::models::UserAnalysis;

#[async_trait]
pub trait AuthenticationHandler: Send + Sync {
    async fn handle_prompt(&self, prompt: AuthenticationPrompt) -> Result<AuthenticationResponse, Error>;
}

/// Every platform's authenticator must implement these methods.
#[async_trait]
pub trait PlatformAuthenticator: Send {
    async fn initialize(&mut self) -> Result<(), Error>;
    async fn start_authentication(&mut self) -> Result<AuthenticationPrompt, Error>;
    async fn complete_authentication(
        &mut self,
        response: AuthenticationResponse
    ) -> Result<PlatformCredential, Error>;
    async fn refresh(&mut self, credential: &PlatformCredential)
                     -> Result<PlatformCredential, Error>;
    async fn validate(&self, credential: &PlatformCredential) -> Result<bool, Error>;
    async fn revoke(&mut self, credential: &PlatformCredential) -> Result<(), Error>;
    fn set_is_broadcaster(&mut self, val: bool);
    fn set_is_teammate(&mut self, val: bool);

    fn set_is_bot(&mut self, _val: bool) {}
}

#[async_trait]
pub trait UserManager: Send + Sync {
    /// Looks up or creates a user record for (platform, platform_user_id).
    async fn get_or_create_user(
        &self,
        platform: Platform,
        platform_user_id: &str,
        platform_username: Option<&str>,
    ) -> Result<User, Error>;

    /// Now takes a Uuid instead of &str:
    async fn get_or_create_user_analysis(&self, user_id: Uuid) -> Result<UserAnalysis, Error>;

    async fn update_user_activity(&self, user_id: &str, new_username: Option<&str>)
                                  -> Result<(), Error>;
}