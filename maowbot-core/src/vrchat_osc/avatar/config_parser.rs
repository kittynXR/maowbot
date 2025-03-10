// File: maowbot-core/src/vrchat_osc/avatar/config_parser.rs
//! Code to parse VRChat's auto-generated avatar .json files. Typically found in:
//!   C:\Users\<User>\AppData\LocalLow\VRChat\VRChat\OSC\usr_<id>\Avatars\avtr_<id>.json

use serde::{Deserialize, Serialize};
use std::fs;
use crate::Error;

/// The top-level structure in VRChat's config file (usually).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AvatarOscConfig {
    pub id: String,
    pub name: String,
    pub parameters: Vec<AvatarParameterEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AvatarParameterEntry {
    pub name: String,
    #[serde(default)]
    pub input: Option<ParameterInputSpec>,
    #[serde(default)]
    pub output: Option<ParameterOutputSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParameterInputSpec {
    pub address: String,
    pub r#type: String, // "Int", "Bool", or "Float"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParameterOutputSpec {
    pub address: String,
    pub r#type: String, // "Int", "Bool", or "Float"
}

/// Parse the JSON file on disk that VRChat generated for an avatar.
/// Returns an `AvatarOscConfig` if successful.
pub fn parse_avatar_osc_config(path: &str) -> Result<AvatarOscConfig, Error> {
    let contents = fs::read_to_string(path)
        .map_err(|e| Error::Platform(format!("Failed to read avatar .json: {e}")))?;
    let parsed: AvatarOscConfig = serde_json::from_str(&contents)
        .map_err(|e| Error::Platform(format!("Failed to parse avatar .json: {e}")))?;
    Ok(parsed)
}
