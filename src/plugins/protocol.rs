// src/plugins/protocol.rs
use serde::{Deserialize, Serialize};
use crate::plugins::capabilities::{
    PluginCapability, RequestedCapabilities, GrantedCapabilities
};

/// Represents events the bot sends to plugins.
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type", content = "data")] // for more structured JSON
pub enum BotToPlugin {
    /// The bot welcomes the plugin once it connects & authenticates.
    Welcome {
        bot_name: String,
    },

    /// The server indicates a passphrase check failed (disconnected soon after).
    AuthError {
        reason: String,
    },

    /// Periodic “heartbeat” or “tick” event
    Tick,

    /// A chat message arrived on the specified platform/channel
    ChatMessage {
        platform: String,
        channel: String,
        user: String,
        text: String,
    },

    /// Server responds with the status summary
    StatusResponse {
        connected_plugins: Vec<String>,
        server_uptime: u64,
    },

    /// Server responds to a capability request
    CapabilityResponse(GrantedCapabilities),

    /// [Optional] If the plugin is forcibly removed or shut down
    ForceDisconnect {
        reason: String,
    },
}

/// Represents messages the plugin sends to the bot.
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")] // structured JSON
pub enum PluginToBot {
    /// The plugin can send a log message or diagnostic
    LogMessage {
        text: String,
    },

    /// Plugin introduces itself.
    /// This is the first message, including optional passphrase.
    Hello {
        plugin_name: String,
        passphrase: Option<String>,
    },

    /// The plugin requests a status summary
    RequestStatus,

    /// The plugin requests certain capabilities
    RequestCapabilities(RequestedCapabilities),

    /// The plugin requests that the bot shutdown
    Shutdown,

    /// The plugin requests to switch scenes
    SwitchScene {
        scene_name: String,
    },

    /// The plugin wants to send a chat message
    SendChat {
        channel: String,
        text: String,
    },
}
