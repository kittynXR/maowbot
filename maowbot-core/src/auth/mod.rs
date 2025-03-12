use async_trait::async_trait;
use maowbot_common::models::auth::{AuthenticationResponse, AuthenticationPrompt};
use maowbot_common::traits::auth_traits::AuthenticationHandler;


pub mod manager;
pub mod user_manager;
pub mod callback_server;

use crate::Error;


#[derive(Default)]
pub struct StubAuthHandler;

#[async_trait]
impl AuthenticationHandler for StubAuthHandler {
    async fn handle_prompt(&self, _prompt: AuthenticationPrompt) -> Result<AuthenticationResponse, Error> {
        // Always just return "None"
        Ok(AuthenticationResponse::None)
    }
}