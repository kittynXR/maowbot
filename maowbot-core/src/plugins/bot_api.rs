// File: maowbot-core/src/plugins/bot_api.rs

use std::collections::HashMap;
use std::format;
use crate::{Error, models::Platform, models::PlatformCredential, models::User};
use async_trait::async_trait;
use uuid::Uuid;
use crate::repositories::postgres::UserRepository;

use tokio::sync::mpsc;
use crate::eventbus::BotEvent;

#[derive(Debug)]
pub struct AccountStatus {
    /// The underlying platform name (e.g. "twitch", "discord", "vrchat", etc.)
    pub platform: String,
    /// A display string for the user. Typically the global_username if available, otherwise user_id.
    pub account_name: String,
    /// Whether the bot’s runtime for this platform+account is currently running/connected.
    pub is_connected: bool,
}

/// Status data reported by the bot to the plugin(s).
#[derive(Debug)]
pub struct StatusData {
    pub connected_plugins: Vec<String>,
    pub uptime_seconds: u64,
    pub account_statuses: Vec<AccountStatus>,
}

/// Represents one platform config row. Usually only one per platform.
#[derive(Debug, Clone)]
pub struct PlatformConfigData {
    pub platform_config_id: Uuid,
    pub platform: String,
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

    /// Create a new user with the specified UUID as `user_id` and display_name as `global_username`.
    async fn create_user(&self, new_user_id: Uuid, display_name: &str) -> Result<(), Error>;

    /// Remove a user (by UUID).
    async fn remove_user(&self, user_id: Uuid) -> Result<(), Error>;

    /// Get a user by UUID.
    async fn get_user(&self, user_id: Uuid) -> Result<Option<User>, Error>;

    /// Update user’s active status by UUID.
    async fn update_user_active(&self, user_id: Uuid, is_active: bool) -> Result<(), Error>;

    /// Searches users by some textual query (may remain string‐based).
    async fn search_users(&self, query: &str) -> Result<Vec<User>, Error>;
    async fn find_user_by_name(&self, name: &str) -> Result<User, Error>;
    /// Step 1 of OAuth or other flows: returns a URL or instructions.
    async fn begin_auth_flow(&self, platform: Platform, is_bot: bool) -> Result<String, Error>;

    /// Complete the flow with code, but **no** user_id – you might store with an empty or default user field.
    async fn complete_auth_flow(
        &self,
        platform: Platform,
        code: String
    ) -> Result<PlatformCredential, Error>;

    /// Complete the flow with code **for a specific user** (by UUID).
    async fn complete_auth_flow_for_user(
        &self,
        platform: Platform,
        code: String,
        user_id: Uuid
    ) -> Result<PlatformCredential, Error>;

    async fn complete_auth_flow_for_user_multi(
        &self,
        platform: Platform,
        user_id: Uuid,
        keys: HashMap<String, String>,
    ) -> Result<PlatformCredential, Error>;
    async fn complete_auth_flow_for_user_2fa(
        &self,
        platform: Platform,
        code: String,
        user_id: Uuid
    ) -> Result<PlatformCredential, Error>;

    /// Revoke credentials for a user (by UUID).
    async fn revoke_credentials(
        &self,
        platform: Platform,
        user_id: String
    ) -> Result<(), Error>;

    async fn refresh_credentials(
        &self,
        platform: Platform,
        user_id: String
    ) -> Result<PlatformCredential, Error>;

    /// List all credentials (optionally filtered by platform).
    async fn list_credentials(
        &self,
        maybe_platform: Option<Platform>
    ) -> Result<Vec<PlatformCredential>, Error>;

    /// Create or update the stored client_id/secret for a platform.
    async fn create_platform_config(
        &self,
        platform: Platform,
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

    async fn remove_platform_config(
        &self,
        platform_config_id: &str
    ) -> Result<(), Error>;
    async fn start_platform_runtime(&self, platform: &str, account_name: &str) -> Result<(), Error>;
    async fn stop_platform_runtime(&self, platform: &str, account_name: &str) -> Result<(), Error>;
    async fn get_bot_config_value(&self, key: &str) -> Result<Option<String>, Error>;
    async fn set_bot_config_value(&self, key: &str, value: &str) -> Result<(), Error>;

    async fn subscribe_chat_events(&self, buffer_size: Option<usize>) -> mpsc::Receiver<BotEvent>;
    async fn list_config(&self) -> Result<Vec<(String, String)>, Error>;
    async fn join_twitch_irc_channel(&self, account_name: &str, channel: &str) -> Result<(), Error>;
    async fn part_twitch_irc_channel(&self, account_name: &str, channel: &str) -> Result<(), Error>;
    async fn send_twitch_irc_message(&self, account_name: &str, channel: &str, text: &str) -> Result<(), Error>;
    async fn store_credential(&self, cred: PlatformCredential) -> Result<(), Error>;
}