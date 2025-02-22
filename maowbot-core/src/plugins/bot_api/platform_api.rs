//! plugins/bot_api/platform_api.rs
//!
//! Sub-trait for platform-level config, starting/stopping runtimes, global config key/values, etc.

use crate::Error;
use crate::models::Platform;
use async_trait::async_trait;

/// Represents one platform config record from the DB (client_id, secret, etc.).
#[derive(Debug, Clone)]
pub struct PlatformConfigData {
    pub platform_config_id: uuid::Uuid,
    pub platform: String,
    pub client_id: Option<String>,
    pub client_secret: Option<String>,
}

/// Sub-trait that deals with platform config (OAuth client_id) and running/stoping connections.
#[async_trait]
pub trait PlatformApi: Send + Sync {
    /// Insert or update a row in “platform_config” for the given platform (client_id, secret).
    async fn create_platform_config(
        &self,
        platform: Platform,
        client_id: String,
        client_secret: Option<String>
    ) -> Result<(), Error>;

    /// Counts how many platform_config rows exist for the given platform string (case-insensitive).
    async fn count_platform_configs_for_platform(
        &self,
        platform_str: String
    ) -> Result<usize, Error>;

    /// Lists all platform_config rows (or just for one platform if `maybe_platform` is provided).
    async fn list_platform_configs(
        &self,
        maybe_platform: Option<&str>
    ) -> Result<Vec<PlatformConfigData>, Error>;

    /// Removes a platform_config row by its UUID (passed as string).
    async fn remove_platform_config(
        &self,
        platform_config_id: &str
    ) -> Result<(), Error>;

    /// Starts the bot’s runtime for a given platform + account.
    async fn start_platform_runtime(&self, platform: &str, account_name: &str) -> Result<(), Error>;

    /// Stops the bot’s runtime for a given platform + account.
    async fn stop_platform_runtime(&self, platform: &str, account_name: &str) -> Result<(), Error>;

    /// Gets a value from the “bot_config” table (like a key-value store).
    async fn get_bot_config_value(&self, key: &str) -> Result<Option<String>, Error>;

    /// Sets a value in the “bot_config” table.
    async fn set_bot_config_value(&self, key: &str, value: &str) -> Result<(), Error>;
}