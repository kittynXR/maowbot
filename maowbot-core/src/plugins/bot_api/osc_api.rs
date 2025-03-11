//! maowbot-core/src/plugins/bot_api/osc_api.rs
//!
//! Sub-trait for OSC-related commands (start, stop, status, chatbox, etc.).

use async_trait::async_trait;
use crate::Error;

/// Information about the current OSC status, used by `osc_status()`.
#[derive(Debug)]
pub struct OscStatus {
    pub is_running: bool,
    pub listening_port: Option<u16>,
    pub is_oscquery_running: bool,
    pub oscquery_port: Option<u16>,

    /// Optionally, any discovered local OSCQuery peers, if we've run a discovery check.
    pub discovered_peers: Vec<String>,
}

/// Trait for controlling the OSC manager (start/stop/restart) and sending chatbox messages.
#[async_trait]
pub trait OscApi: Send + Sync {
    /// Starts the OSC system (UDP server, OSCQuery HTTP, etc.).
    async fn osc_start(&self) -> Result<(), Error>;

    /// Stops the OSC system.
    async fn osc_stop(&self) -> Result<(), Error>;

    /// Restarts the OSC system (stop+start).
    async fn osc_restart(&self) -> Result<(), Error> {
        self.osc_stop().await?;
        self.osc_start().await
    }

    /// Returns the current OSC status (running or not, port, etc.).
    async fn osc_status(&self) -> Result<OscStatus, Error>;

    /// Sends a single chatbox message to VRChat via OSC (if running).
    async fn osc_chatbox(&self, message: &str) -> Result<(), Error>;

    /// Optionally discover local OSCQuery peers; returns their service names or addresses.
    async fn osc_discover_peers(&self) -> Result<Vec<String>, Error>;
}
