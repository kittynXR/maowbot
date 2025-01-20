// src/plugins/capabilities.rs

//! Defines plugin capability types and logic.

use serde::{Deserialize, Serialize};

/// A top-level enum describing capabilities that a plugin can request.
/// In real usage, subdivide these or refine them as your needs grow.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PluginCapability {
    /// The ability to receive chat events from the bot.
    ReceiveChatEvents,
    /// The ability to send chat messages.
    SendChat,
    /// The ability to switch streaming scenes (e.g. OBS).
    SceneManagement,
    /// The ability to moderate chat (ban/unban).
    ChatModeration,
}

/// Summary of which capabilities a plugin requests and is granted.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestedCapabilities {
    pub requested: Vec<PluginCapability>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrantedCapabilities {
    pub granted: Vec<PluginCapability>,
    pub denied: Vec<PluginCapability>,
}
