// maowbot-osc/src/vrchat/mod.rs
//! VRChat-specific logic, including discovering VRChat's OSCQuery service
//! and scanning for the VRChat Avatars folder on disk.
pub mod avatar;
pub mod toggles;
pub mod chatbox;
pub mod avatar_watcher;
use std::fs;
use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};
use tracing::{info, warn, debug, error};
use crate::oscquery::client::OscQueryClient;
use crate::{OscError, Result, VRChatConnectionInfo};
use crate::oscquery::mdns::service::MdnsService;
use tokio::time;
pub use avatar_watcher::AvatarWatcher;
use crate::oscquery::models::OSCQueryHostInfo;
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct VrchatAvatarConfig {
    pub id: String,
    pub name: String,
    pub parameters: Vec<VrchatParameterConfig>,
}
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
    pub param_type: String,
}
pub fn parse_vrchat_avatar_config<P: AsRef<Path>>(path: P) -> Result<VrchatAvatarConfig> {
    let p = path.as_ref();
    if !p.exists() {
        return Err(OscError::AvatarConfigError(format!("File does not exist: {}", p.display())));
    }
    let bytes = fs::read(p)
        .map_err(|e| OscError::AvatarConfigError(format!("Could not read file {}: {e}", p.display())))?;
    if bytes.is_empty() {
        return Err(OscError::AvatarConfigError(format!("File is empty: {}", p.display())));
    }
    // Strip UTF-8 BOM if present
    let content = if bytes.len() >= 3 && bytes[0] == 0xEF && bytes[1] == 0xBB && bytes[2] == 0xBF {
        &bytes[3..]
    } else {
        &bytes[..]
    };
    serde_json::from_slice(content).map_err(|e| {
        let preview_len = std::cmp::min(40, content.len());
        let preview_str = String::from_utf8_lossy(&content[..preview_len]);
        error!("JSON parse error in {}: {} (first bytes: '{}')", p.display(), e, preview_str);
        OscError::AvatarConfigError(format!("JSON parse error: {e}"))
    })
}
pub fn load_all_vrchat_avatar_configs<P: AsRef<Path>>(dir: P) -> Vec<VrchatAvatarConfig> {
    let mut results = Vec::new();
    let dir = dir.as_ref();
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(ext) = path.extension() {
                if ext.to_ascii_lowercase() == "json" {
                    match parse_vrchat_avatar_config(&path) {
                        Ok(cfg) => results.push(cfg),
                        Err(e) => {
                            eprintln!("Failed to parse {:?}: {e}", path);
                        }
                    }
                }
            }
        }
    }
    results
}
pub fn get_vrchat_osc_dir() -> Option<PathBuf> {
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
            let path = home.join("Library").join("Application Support")
                .join("com.vrchat.VRChat").join("OSC");
            if path.exists() {
                return Some(path);
            }
        }
    }
    #[cfg(target_os = "linux")]
    {
        if let Some(home) = dirs::home_dir() {
            // Linux: ~/.local/share/VRChat/VRChat/OSC
            let path = home.join(".local").join("share")
                .join("VRChat").join("VRChat").join("OSC");
            if path.exists() {
                return Some(path);
            }
        }
    }
    None
}
pub fn get_vrchat_avatar_dir() -> Option<PathBuf> {
    if let Some(osc_dir) = get_vrchat_osc_dir() {
        if let Ok(entries) = fs::read_dir(&osc_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                    if name.starts_with("usr_") {
                        let avatar_sub = path.join("Avatars");
                        if avatar_sub.exists() && avatar_sub.is_dir() {
                            return Some(avatar_sub);
                        }
                    }
                }
            }
        }
    }
    None
}
/// We define a struct that returns both ports from VRChat’s two mDNS announcements.
#[derive(Debug, Clone)]
pub struct VrchatDiscoveredPorts {
    /// The host or IP from the discovered record(s). Typically 127.0.0.1 or your LAN IP.
    pub oscquery_host: String,
    /// The TCP port for VRChat’s OSCQuery.
    pub oscquery_port: u16,
    /// The UDP port VRChat listens on (the port we should send to). Typically 9000 or ephemeral.
    pub osc_send_port: u16,
    /// The UDP port VRChat sends from. Typically 9001 or ephemeral.
    pub osc_receive_port: u16,
}
/// Attempt to discover VRChat's _osc._udp and _oscjson._tcp services via mDNS.
/// This function starts the mDNS query listener, queries for VRChat services,
/// and returns connection info if found; otherwise, it uses fallback ports.
pub async fn discover_vrchat() -> Result<Option<VRChatConnectionInfo>> {
    info!("Starting VRChat mDNS discovery (this runs in background)...");
    // Create your custom mDNS service
    let mut mdns = MdnsService::new()?;
    // Start the query listener to process inbound response packets
    mdns.start_query_listener();
    // Allow a bit of time for passive discovery before sending queries
    tokio::time::sleep(time::Duration::from_millis(200)).await;
    // First query for OSC UDP service
    debug!("Querying for _osc._udp.local with VRChat-Client filter");
    let discovered = mdns.query_for_service("_osc._udp.local", Some("VRChat-Client"))?;
    let osc_services: Vec<_> = discovered.into_iter()
        .filter(|svc| svc.service_name.starts_with("VRChat-Client"))
        .collect();
    debug!("Found {} VRChat OSC UDP services", osc_services.len());
    // Then query for OSCQuery TCP service
    debug!("Querying for _oscjson._tcp.local with VRChat-Client filter");
    let discovered_json = mdns.query_for_service("_oscjson._tcp.local", Some("VRChat-Client"))?;
    let oscquery_services: Vec<_> = discovered_json.into_iter()
        .filter(|svc| svc.service_name.starts_with("VRChat-Client"))
        .collect();
    debug!("Found {} VRChat OSCQuery TCP services", oscquery_services.len());
    // Combine the results
    mdns.stop();
    if osc_services.is_empty() && oscquery_services.is_empty() {
        warn!("No VRChat mDNS entries found");
        return Ok(None);
    }
    // Try to find the OSC UDP service
    let osc_port = if !osc_services.is_empty() {
        let first = &osc_services[0];
        info!("Found VRChat UDP service: {} => port {} on {}",
            first.service_name, first.port, first.address);
        first.port
    } else {
        info!("No VRChat OSC service found, falling back to default port 9000");
        9000
    };
    // Try to find the OSCQuery TCP service
    let (oscquery_port, oscquery_host) = if !oscquery_services.is_empty() {
        let first = &oscquery_services[0];
        info!("Found VRChat OSCQuery service: {} => port {} on {}",
            first.service_name, first.port, first.address);
        (first.port, first.address.to_string())
    } else if !osc_services.is_empty() {
        // Use the host from OSC service but default port for OSCQuery
        let ip = osc_services[0].address.to_string();
        info!("No VRChat OSCQuery service found, using IP {} and default port 0", ip);
        (0, ip)
    } else {
        info!("No VRChat services found, using localhost and default port 0");
        (0, "127.0.0.1".to_string())
    };
    // For receive port, we'll get it from host_info query if available, otherwise use 9001
    let info = VRChatConnectionInfo {
        oscquery_host,
        oscquery_port,
        osc_send_port: osc_port,
        osc_receive_port: 0, // We'll let the caller determine this via oscquery or default to 9001
    };
    Ok(Some(info))
}
pub async fn query_vrchat_oscquery(
    _client: &crate::oscquery::client::OscQueryClient,
    host: &str,
    port: u16,
    _filter: Option<&str>,
) -> Result<Option<(String, u16)>> {
    let url = format!("http://{}:{}/HOST_INFO", host, port);
    let response = reqwest::get(&url)
        .await
        .map_err(|e| OscError::OscQueryError(format!("HTTP error: {}", e)))?;
    if response.status().is_success() {
        let info: OSCQueryHostInfo = response.json()
            .await
            .map_err(|e| OscError::OscQueryError(format!("JSON parse error: {}", e)))?;
        Ok(Some((info.OSC_IP, info.OSC_PORT)))
    } else {
        Ok(None)
    }
}
