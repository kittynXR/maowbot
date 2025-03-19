// File: maowbot-osc/src/vrchat/mod.rs
//! maowbot-osc/src/vrchat/mod.rs
//!
//! Various VRChat-specific code for parsing avatar JSON files,
//! controlling toggles, sending/receiving chat messages, etc.

pub mod avatar;
pub mod toggles;
pub mod chatbox;

use crate::{OscError, Result};
use std::path::{Path, PathBuf};
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

    // Check if file exists
    if !p.exists() {
        return Err(OscError::AvatarConfigError(format!("File does not exist: {}", p.display())));
    }

    // Read file as bytes first to check if it's empty
    let bytes = match fs::read(p) {
        Ok(b) => b,
        Err(e) => {
            return Err(OscError::AvatarConfigError(format!(
                "Could not read file {}: {e}",
                p.display()
            )));
        }
    };

    if bytes.is_empty() {
        return Err(OscError::AvatarConfigError(format!("File is empty: {}", p.display())));
    }

    // Try to parse the JSON with more detailed error reporting
    match serde_json::from_slice::<VrchatAvatarConfig>(&bytes) {
        Ok(cfg) => Ok(cfg),
        Err(e) => {
            // Log the first few bytes as hex for debugging
            let preview_len = std::cmp::min(40, bytes.len());
            let preview = &bytes[..preview_len];
            let preview_text = String::from_utf8_lossy(preview);

            tracing::error!(
                "JSON parse error for {}: {} (first bytes: '{}')",
                p.display(), e, preview_text
            );

            // VRChat sometimes writes empty or partial files, try removing BOM if present
            if bytes.len() >= 3 && bytes[0] == 0xEF && bytes[1] == 0xBB && bytes[2] == 0xBF {
                // Try parsing without the BOM
                match serde_json::from_slice::<VrchatAvatarConfig>(&bytes[3..]) {
                    Ok(cfg) => {
                        tracing::info!("Successfully parsed after removing BOM marker");
                        return Ok(cfg);
                    }
                    Err(_) => {
                        // Still failed, continue to error
                    }
                }
            }

            Err(OscError::AvatarConfigError(format!("JSON parse error: {}", e)))
        }
    }
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

// Re-export the new 'avatar_watcher' and 'avatar_toggle_menu'
pub mod avatar_watcher;

pub use avatar_watcher::AvatarWatcher;

/// Get the path to VRChat's OSC output directory
pub fn get_vrchat_osc_dir() -> Option<PathBuf> {
    // Default locations by platform
    #[cfg(target_os = "windows")]
    {
        if let Some(local_low) = dirs::data_local_dir() {
            if let Some(parent) = local_low.parent() {
                // Windows: %LOCALAPPDATA%\LocalLow\VRChat\VRChat\OSC
                let path = parent.join("LocalLow").join("VRChat").join("VRChat").join("OSC");

                if path.exists() {
                    return Some(path);
                }
            }
        }
    }

    #[cfg(target_os = "macos")]
    {
        if let Some(home) = dirs::home_dir() {
            // macOS: ~/Library/Application Support/com.vrchat.VRChat/OSC
            let path = home.join("Library")
                .join("Application Support")
                .join("com.vrchat.VRChat")
                .join("OSC");

            if path.exists() {
                return Some(path);
            }
        }
    }

    #[cfg(target_os = "linux")]
    {
        if let Some(home) = dirs::home_dir() {
            // Linux: ~/.local/share/VRChat/VRChat/OSC
            let path = home.join(".local")
                .join("share")
                .join("VRChat")
                .join("VRChat")
                .join("OSC");

            if path.exists() {
                return Some(path);
            }
        }
    }

    None
}

/// Get the path to VRChat's avatar folder for the current user
pub fn get_vrchat_avatar_dir() -> Option<PathBuf> {
    get_vrchat_osc_dir().and_then(|osc_dir| {
        // Look for any user folder (usr_*)
        if let Ok(entries) = fs::read_dir(&osc_dir) {
            for entry in entries {
                if let Ok(entry) = entry {
                    let path = entry.path();
                    if path.is_dir() && path.file_name()
                        .and_then(|n| n.to_str())
                        .map(|s| s.starts_with("usr_"))
                        .unwrap_or(false)
                    {
                        // Found user folder, look for Avatars subfolder
                        let avatar_dir = path.join("Avatars");
                        if avatar_dir.exists() && avatar_dir.is_dir() {
                            return Some(avatar_dir);
                        }
                    }
                }
            }
        }
        None
    })
}