//! Data structures / enumerations for use in our OSCQuery server or client.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Enumerates how we can allow reading/writing to a given OSC path via OSCQuery.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum OSCMethodAccessType {
    /// External apps can only write to this path
    Write,
    /// External apps can only read from this path
    Read,
    /// External apps can both read/write
    ReadWrite,
}

/// Basic OSC value types
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum OSCMethodValueType {
    Bool,
    Int,
    Float,
    String,
}

impl OSCMethodValueType {
    pub fn osc_type_str(&self) -> &str {
        match self {
            OSCMethodValueType::Bool => "F",   // VRChat doesn't strictly require single-letter codes, but commonly "T/F"
            OSCMethodValueType::Int => "i",
            OSCMethodValueType::Float => "f",
            OSCMethodValueType::String => "s",
        }
    }
}

/// Advertised “method” or “address” in our OSCQuery system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OSCMethod {
    pub address: String,
    pub access_type: OSCMethodAccessType,
    /// If read access is allowed, we might advertise a type
    pub value_type: Option<OSCMethodValueType>,
    /// If we want to show a “current value” in queries
    pub value: Option<String>,
    /// Optional human-friendly description
    pub description: Option<String>,
}

/// Host info response for /HOST_INFO
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OSCQueryHostInfo {
    pub NAME: String,
    pub OSC_IP: String,
    pub OSC_PORT: u16,
    pub OSC_TRANSPORT: String,
    pub EXTENSIONS: HashMap<String, bool>,
}

/// Node in the OSCQuery “directory” tree
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OSCQueryNode {
    /// e.g. "Root Container"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub DESCRIPTION: Option<String>,

    pub FULL_PATH: String,

    /// Bitmask for read/write. 1=Read, 2=Write, 3=Read+Write
    pub ACCESS: u8,

    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub CONTENTS: HashMap<String, OSCQueryNode>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub TYPE: Option<String>,

    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub VALUE: Vec<serde_json::Value>,
}
