//! plugins/bot_api/plugin_api.rs
//!
//! Sub-trait for high-level plugin-related methods, global status, etc.

use crate::Error;
use crate::eventbus::BotEvent;
use async_trait::async_trait;
use tokio::sync::mpsc;

/// Represents one account’s status in the bot’s system.
#[derive(Debug)]
pub struct AccountStatus {
    /// The underlying platform name (e.g. "twitch", "discord", "vrchat", etc.)
    pub platform: String,
    /// A display string for the user. Typically the user’s global_username if available.
    pub account_name: String,
    /// Whether the bot’s runtime for this platform+account is currently running/connected.
    pub is_connected: bool,
}

/// High-level status data reported by the bot to the plugin(s) or the TUI.
#[derive(Debug)]
pub struct StatusData {
    pub connected_plugins: Vec<String>,
    pub uptime_seconds: u64,
    pub account_statuses: Vec<AccountStatus>,
}

/// Sub-trait that deals with plugin listing, global status, shutdown, etc.
#[async_trait]
pub trait PluginApi: Send + Sync {
    /// Returns a list of plugin names. You might label them as “(disabled)” in your logic if wanted.
    async fn list_plugins(&self) -> Vec<String>;

    /// Returns an overall `StatusData` snapshot (which plugins are connected, accounts connected, etc.).
    async fn status(&self) -> StatusData;

    /// Requests that the entire bot shuts down gracefully.
    async fn shutdown(&self);

    /// Toggles a plugin by name: if `enable == true`, enable it; if false, disable it.
    async fn toggle_plugin(&self, plugin_name: &str, enable: bool) -> Result<(), Error>;

    /// Permanently removes a plugin from the system (unloads and deletes from JSON).
    async fn remove_plugin(&self, plugin_name: &str) -> Result<(), Error>;

    /// Subscribe to chat events from the global event bus. 
    /// Returns an MPSC receiver that yields `BotEvent::ChatMessage`.
    async fn subscribe_chat_events(&self, buffer_size: Option<usize>) -> mpsc::Receiver<BotEvent>;

    /// Lists all config key/value pairs from some “bot_config” table (if implemented).
    async fn list_config(&self) -> Result<Vec<(String, String)>, Error>;
}