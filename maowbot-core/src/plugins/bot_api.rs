// maowbot-core/src/plugins/bot_api.rs

use crate::Error;

#[derive(Debug)]
pub struct StatusData {
    pub connected_plugins: Vec<String>,
    pub uptime_seconds: u64,
}

pub trait BotApi: Send + Sync {
    fn list_plugins(&self) -> Vec<String>;
    fn status(&self) -> StatusData;
    fn shutdown(&self);
    /// Toggle a plugin on/off synchronously.
    fn toggle_plugin(&self, plugin_name: &str, enable: bool) -> Result<(), Error>;
}
