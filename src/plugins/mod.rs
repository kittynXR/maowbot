// File: src/plugins/mod.rs

pub mod protocol;
pub mod manager;
pub mod capabilities;
pub mod tui_plugin;

pub use protocol::BotToPlugin;
pub use protocol::PluginToBot;