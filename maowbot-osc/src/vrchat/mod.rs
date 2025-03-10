// File: maowbot-osc/src/vrchat/mod.rs
//! maowbot-osc/src/vrchat/mod.rs
//!
//! Various VRChat-specific code for parsing avatar JSON files,
//! controlling toggles, sending/receiving chat messages, etc.

pub mod avatar;
pub mod toggles;
pub mod chatbox;

use crate::{OscError, Result};
use std::path::Path;
use std::fs;
use serde::{Deserialize, Serialize};

/// A minimal definition matching the structure of VRChat's generated JSON
/// (the "id", "name", "parameters" list).
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct VrchatAvatarConfig {
    pub id: String,
    pub name: String,
    pub parameters: Vec<VrchatParameterConfig>,
}

/// Each parameter includes an input and/or output spec.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct VrchatParameterConfig {
    pub name: String,
    #[serde(default)]
    pub input: Option<VrchatParamEndpoint>,
    #[serde(default)]
    pub output: Option<VrchatParamEndpoint>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct VrchatParamEndpoint {
    pub address: String,
    #[serde(rename = "type")]
    pub param_type: String, // "Int", "Float", or "Bool"
}

/// A helper that attempts to parse a VRChat avatar config JSON file.
/// (e.g. `C:\Users\YOU\AppData\LocalLow\VRChat\VRChat\OSC\usr_\Avatars\avtr_\*.json`).
pub fn parse_vrchat_avatar_config<P: AsRef<Path>>(path: P) -> Result<VrchatAvatarConfig> {
    let p = path.as_ref();
    let data = fs::read_to_string(p)
        .map_err(|e| OscError::AvatarConfigError(format!("Could not read file {}: {e}", p.display())))?;

    let cfg: VrchatAvatarConfig = serde_json::from_str(&data)
        .map_err(|e| OscError::AvatarConfigError(format!("JSON parse error: {e}")))?;
    Ok(cfg)
}

/// Utility to scan a directory for all `.json` files and parse them as VRChat avatar configs.
/// Returns a vector of successfully parsed configs. Ignores parse failures (just logs them).
pub fn load_all_vrchat_avatar_configs<P: AsRef<Path>>(dir: P) -> Vec<VrchatAvatarConfig> {
    let mut results = Vec::new();
    let dir = dir.as_ref();
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries {
            if let Ok(ent) = entry {
                let path = ent.path();
                if path.extension().map(|s| s.to_string_lossy()) == Some("json".into()) {
                    match parse_vrchat_avatar_config(&path) {
                        Ok(cfg) => {
                            results.push(cfg);
                        }
                        Err(e) => {
                            eprintln!("Failed to parse {:?}: {}", path, e);
                        }
                    }
                }
            }
        }
    }
    results
}

// NEW: Re-export the new 'avatar_watcher' and 'avatar_toggle_menu'
pub mod avatar_watcher;

pub use avatar_watcher::AvatarWatcher;

