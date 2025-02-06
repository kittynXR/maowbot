// maowbot-core/src/plugins/bot_api.rs
use crate::{Error, models::Platform, models::PlatformCredential};
use async_trait::async_trait;

#[derive(Debug)]
pub struct StatusData {
    pub connected_plugins: Vec<String>,
    pub uptime_seconds: u64,
}

#[async_trait]
pub trait BotApi: Send + Sync {
    async fn list_plugins(&self) -> Vec<String>;
    async fn status(&self) -> StatusData;
    async fn shutdown(&self);

    async fn toggle_plugin(&self, plugin_name: &str, enable: bool) -> Result<(), Error>;
    async fn remove_plugin(&self, plugin_name: &str) -> Result<(), Error>;

    // -------------- NEW AUTH-FLOW METHODS ------------------

    /// Step 1 of the flow: returns a `Browser { url }` prompt (or whatever the authenticator yields)
    /// after calling the underlying `Authenticator::start_authentication()`
    /// (plus any initialization, is_bot flag, etc.).
    async fn begin_auth_flow(
        &self,
        platform: Platform,
        is_bot: bool
    ) -> Result<String, Error>;

    /// Step 2 of the flow: once we have the `code` from the callback server,
    /// we finalize the OAuth token exchange and store the resulting credential.
    async fn complete_auth_flow(
        &self,
        platform: Platform,
        code: String
    ) -> Result<PlatformCredential, Error>;

    /// Revoke credentials for a given user on a given platform.
    async fn revoke_credentials(
        &self,
        platform: Platform,
        user_id: &str
    ) -> Result<(), Error>;

    /// Potentially also list credentials. For brevity we return them all or filter by platform.
    async fn list_credentials(
        &self,
        maybe_platform: Option<Platform>
    ) -> Result<Vec<PlatformCredential>, Error>;
}
