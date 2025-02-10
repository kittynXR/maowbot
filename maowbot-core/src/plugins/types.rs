// src/plugins/types.rs
use serde::{Deserialize, Serialize};

/// Which kind of plugin we’re dealing with (GRPC or a .so/.dll).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PluginType {
    DynamicLib { path: String },
    Grpc,
}

/// Record for a plugin in our saved “plugins_state.json”.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginRecord {
    pub name: String,
    pub plugin_type: PluginType,
    pub enabled: bool,
}

/// A small JSON file that persists all the plugin records across restarts.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct PluginStatesFile {
    pub plugins: Vec<PluginRecord>,
}