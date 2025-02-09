// maowbot-core/src/plugins/bot_api.rs
use crate::{Error, models::Platform, models::PlatformCredential};
use async_trait::async_trait;

#[derive(Debug)]
pub struct StatusData {
    pub connected_plugins: Vec<String>,
    pub uptime_seconds: u64,
}

#[derive(Debug, Clone)]
pub struct PlatformConfigData {
    pub platform_config_id: String,
    pub platform: String,
    pub platform_label: Option<String>,
    pub client_id: Option<String>,
    pub client_secret: Option<String>,
}

#[async_trait]
pub trait BotApi: Send + Sync {
    async fn list_plugins(&self) -> Vec<String>;
    async fn status(&self) -> StatusData;
    async fn shutdown(&self);
    async fn toggle_plugin(&self, plugin_name: &str, enable: bool) -> Result<(), Error>;
    async fn remove_plugin(&self, plugin_name: &str) -> Result<(), Error>;

    /// Begin auth flow using the default label.
    async fn begin_auth_flow(
        &self,
        platform: Platform,
        is_bot: bool
    ) -> Result<String, Error>;

    /// Begin auth flow with a specified label.
    async fn begin_auth_flow_with_label(
        &self,
        platform: Platform,
        is_bot: bool,
        label: &str
    ) -> Result<String, Error>;

    /// Complete the auth flow with the provided code.
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

    /// List stored credentials, optionally filtered by platform.
    async fn list_credentials(
        &self,
        maybe_platform: Option<Platform>
    ) -> Result<Vec<PlatformCredential>, Error>;

    /// Create a new platform configuration (in the platform_config table).
    async fn create_platform_config(
        &self,
        platform: Platform,
        label: &str,
        client_id: String,
        client_secret: Option<String>
    ) -> Result<(), Error>;

    async fn count_platform_configs_for_platform(
        &self,
        platform_str: String
    ) -> Result<usize, Error>;

    async fn list_platform_configs(
        &self,
        maybe_platform: Option<&str>
    ) -> Result<Vec<PlatformConfigData>, Error>;

    /// Removes a single `platform_config` row by ID (the TUI calls this).
    async fn remove_platform_config(
        &self,
        platform_config_id: &str
    ) -> Result<(), Error>;
}
