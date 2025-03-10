//! maowbot-osc/src/vrchat/mod.rs
//!
//! Various VRChat-specific code for parsing avatar JSON files,
//! controlling toggles, sending/receiving chat messages, etc.

pub mod avatar;
pub mod toggles;

use crate::{Result, OscError};
use std::path::{Path, PathBuf};
use std::fs;
use serde::{Deserialize, Serialize};

/// A minimal definition matching the structure of VRChat's generated JSON
/// (the "id", "name", "parameters" list).
#[derive(Debug, Serialize, Deserialize)]
pub struct VrchatAvatarConfig {
    pub id: String,
    pub name: String,
    pub parameters: Vec<VrchatParameterConfig>,
}

/// Each parameter includes an input and/or output spec.
#[derive(Debug, Serialize, Deserialize)]
pub struct VrchatParameterConfig {
    pub name: String,
    #[serde(default)]
    pub input: Option<VrchatParamEndpoint>,
    #[serde(default)]
    pub output: Option<VrchatParamEndpoint>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct VrchatParamEndpoint {
    pub address: String,
    #[serde(rename = "type")]
    pub param_type: String, // "Int", "Float", or "Bool"
}

/// A helper that attempts to parse a VRChat avatar config JSON file
/// ( e.g. `C:\Users\YOU\AppData\LocalLow\VRChat\VRChat\OSC\usr_\Avatars\avtr_\*.json` ).
pub fn parse_vrchat_avatar_config<P: AsRef<Path>>(path: P) -> Result<VrchatAvatarConfig> {
    let p = path.as_ref();
    let data = fs::read_to_string(p)
        .map_err(|e| OscError::AvatarConfigError(format!("Could not read file {}: {e}", p.display())))?;

    let cfg: VrchatAvatarConfig = serde_json::from_str(&data)
        .map_err(|e| OscError::AvatarConfigError(format!("JSON parse error: {e}")))?;
    Ok(cfg)
}
