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