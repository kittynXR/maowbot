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
use crate::oscquery::client::OscQueryClient;
use crate::oscquery::discovery::DiscoveredService;
use std::time::Duration;
use tracing::{info, warn, debug, error};

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

    // Check for BOM and remove it if present
    let content_without_bom = if bytes.len() >= 3 && bytes[0] == 0xEF && bytes[1] == 0xBB && bytes[2] == 0xBF {
        tracing::debug!("BOM detected in {}, removing for parsing", p.display());
        &bytes[3..]
    } else {
        &bytes[..]
    };

    // Try to parse the JSON with more detailed error reporting
    match serde_json::from_slice::<VrchatAvatarConfig>(content_without_bom) {
        Ok(cfg) => Ok(cfg),
        Err(e) => {
            // Log the first few bytes as hex for debugging
            let preview_len = std::cmp::min(40, content_without_bom.len());
            let preview = &content_without_bom[..preview_len];
            let preview_text = String::from_utf8_lossy(preview);

            tracing::error!(
                "JSON parse error for {}: {} (first bytes: '{}')",
                p.display(), e, preview_text
            );

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

/// Discover VRChat's OSCQuery service via mDNS
///
/// This is the primary function for finding VRChat. It uses mDNS to discover
/// VRChat's OSCQuery service, which is how we find out where to send OSC messages.
pub async fn discover_vrchat() -> Result<Option<DiscoveredService>> {
    // Create a discovery service
    let discovery = match crate::oscquery::discovery::OscQueryDiscovery::new() {
        Ok(d) => d,
        Err(e) => {
            error!("Failed to create OSCQuery discovery service: {:?}", e);
            return Err(e);
        }
    };

    // Look specifically for VRChat's service
    info!("Searching for VRChat OSCQuery service via mDNS...");
    match discovery.find_vrchat_service().await {
        Ok(Some(service)) => {
            info!("Found VRChat OSCQuery service: {} on {}:{}",
                  service.name, service.hostname, service.port);
            Ok(Some(service))
        },
        Ok(None) => {
            // If not found with mDNS, try direct connection to default ports
            warn!("VRChat OSCQuery service not found via mDNS, trying default connection");

            // Create a default service using VRChat's standard ports
            let default_service = DiscoveredService {
                name: "VRChat-Default".to_string(),
                hostname: "127.0.0.1".to_string(),
                addr: Some("127.0.0.1".to_string()),
                port: 9000, // Default HTTP OSCQuery port for some VRChat versions
                osc_port: Some(9001), // VRChat sends on 9001
                osc_ip: Some("127.0.0.1".to_string()),
            };

            Ok(Some(default_service))
        },
        Err(e) => {
            error!("Error searching for VRChat OSCQuery service: {:?}", e);
            Err(e)
        }
    }
}

// Function to query VRChat's OSCQuery with hostname and port
pub async fn query_vrchat_oscquery(client: &OscQueryClient, host: &str, port: u16) -> Result<Option<(String, u16)>> {
    // Try to get the host info first to confirm we're talking to VRChat
    let host_info = match client.query_host_info(host, port).await {
        Ok(info) => info,
        Err(e) => {
            debug!("Failed to query host_info from {}: {}", host, e);
            // Try the root endpoint instead
            match client.query_root(host, port).await {
                Ok(root) => root,
                Err(e2) => {
                    warn!("Failed to query OSCQuery root from {}: {}", host, e2);
                    // If all fails, return default values
                    return Ok(Some(("127.0.0.1".to_string(), 9001)));
                }
            }
        }
    };

    // Look for OSC_IP and OSC_PORT in the response
    // First try the direct query keys
    let osc_ip = host_info.get("OSC_IP")
        .and_then(|ip| ip.as_str())
        .map(|s| s.to_string());

    let osc_port = host_info.get("OSC_PORT")
        .and_then(|port| port.as_u64())
        .map(|p| p as u16);

    // If we found both, return them
    if let (Some(ip), Some(port)) = (osc_ip, osc_port) {
        info!("VRChat OSCQuery info: IP={}, PORT={}", ip, port);
        return Ok(Some((ip, port)));
    }

    // If we didn't find them, look for them in CONTENTS section (some versions of VRChat structure it differently)
    if let Some(contents) = host_info.get("CONTENTS") {
        if let Some(obj) = contents.as_object() {
            // Try to find OSC_IP and OSC_PORT in the CONTENTS
            let ip = obj.keys()
                .find(|k| k.to_uppercase() == "OSC_IP")
                .and_then(|k| obj.get(k))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            let port = obj.keys()
                .find(|k| k.to_uppercase() == "OSC_PORT")
                .and_then(|k| obj.get(k))
                .and_then(|v| v.as_u64())
                .map(|p| p as u16);

            if let (Some(ip), Some(port)) = (ip, port) {
                info!("VRChat OSCQuery info (from CONTENTS): IP={}, PORT={}", ip, port);
                return Ok(Some((ip, port)));
            }
        }
    }

    // If we still haven't found them, assume the defaults
    warn!("Could not find OSC_IP and OSC_PORT in VRChat OSCQuery response, using defaults");
    Ok(Some(("127.0.0.1".to_string(), 9001)))
}

// Helper function that works with DiscoveredService
pub async fn query_vrchat_service(client: &OscQueryClient, service: &DiscoveredService) -> Result<Option<(String, u16)>> {
    // If we already have the OSC port from the service discovery, use it
    if let (Some(ip), Some(port)) = (&service.osc_ip, service.osc_port) {
        return Ok(Some((ip.clone(), port)));
    }

    // Otherwise query the OSCQuery server
    let host = service.addr.as_ref().unwrap_or(&service.hostname);
    query_vrchat_oscquery(client, host, service.port).await
}