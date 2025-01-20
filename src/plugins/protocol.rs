// File: src/plugins/protocol.rs

use serde::{Deserialize, Serialize};

/// Represents events the bot sends to plugins.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum BotToPlugin {
    /// An example event: a chat message arrived
    ChatMessage {
        platform: String,
        channel: String,
        user: String,
        text: String,
    },

    /// Periodic “heartbeat” or “tick” event
    Tick,

    /// Bot says “Hello” when plugin connects
    Welcome { bot_name: String },
}

/// Represents messages the plugin sends to the bot.
#[derive(Debug, Serialize, Deserialize)]
pub enum PluginToBot {
    /// The plugin can send a log message or diagnostic
    LogMessage { text: String },

    /// A request for the bot to perform an action
    SendChat { channel: String, text: String },

    /// Plugin “pings” or says hello
    Hello { plugin_name: String },

    /// The plugin can request the bot to shut down or do something else
    Shutdown,
}
