//! maowbot-osc/src/oscquery/client.rs
//!
//! Client for OSCQuery protocol to discover and query remote OSCQuery services

use crate::{Result, OscError};
use serde_json::Value;
use std::net::SocketAddr;
use std::collections::HashMap;
use tokio::time::Duration;
use tracing::{info, debug, error};

/// Represents a discovered OSCQuery service
#[derive(Debug, Clone)]
pub struct DiscoveredOscQueryService {
    pub name: String,
    pub host: String,
    pub http_port: u16,
    pub osc_port: Option<u16>,
    pub properties: HashMap<String, String>,
}

/// Client for OSCQuery protocol
pub struct OscQueryClient {
    pub timeout: Duration,
}

impl OscQueryClient {
    pub fn new() -> Self {
        Self {
            timeout: Duration::from_secs(5),
        }
    }

    /// Query an OSCQuery server for its host info
    pub async fn query_host_info(&self, host: &str, port: u16) -> Result<Value> {
        let url = format!("http://{}:{}/host_info", host, port);
        debug!("Querying OSCQuery host info from {}", url);

        let client = reqwest::Client::new();
        let res = client.get(&url)
            .timeout(self.timeout)
            .send()
            .await
            .map_err(|e| OscError::OscQueryError(format!("Failed to query host info: {}", e)))?;

        let json = res.json::<Value>()
            .await
            .map_err(|e| OscError::OscQueryError(format!("Failed to parse host info response: {}", e)))?;

        debug!("Received host info: {:?}", json);
        Ok(json)
    }

    /// Query the root OSCQuery endpoint for all available OSC addresses
    pub async fn query_root(&self, host: &str, port: u16) -> Result<Value> {
        let url = format!("http://{}:{}/", host, port);
        debug!("Querying OSCQuery root from {}", url);

        let client = reqwest::Client::new();
        let res = client.get(&url)
            .timeout(self.timeout)
            .send()
            .await
            .map_err(|e| OscError::OscQueryError(format!("Failed to query root: {}", e)))?;

        let json = res.json::<Value>()
            .await
            .map_err(|e| OscError::OscQueryError(format!("Failed to parse root response: {}", e)))?;

        debug!("Received OSCQuery root: {:?}", json);
        Ok(json)
    }

    /// Query specific path from OSCQuery server
    pub async fn query_path(&self, host: &str, port: u16, path: &str) -> Result<Value> {
        // Ensure path starts with a slash
        let path = if !path.starts_with('/') {
            format!("/{}", path)
        } else {
            path.to_string()
        };

        let url = format!("http://{}:{}{}", host, port, path);
        debug!("Querying OSCQuery path {} from {}", path, url);

        let client = reqwest::Client::new();
        let res = client.get(&url)
            .timeout(self.timeout)
            .send()
            .await
            .map_err(|e| OscError::OscQueryError(format!("Failed to query path {}: {}", path, e)))?;

        let json = res.json::<Value>()
            .await
            .map_err(|e| OscError::OscQueryError(format!("Failed to parse path response: {}", e)))?;

        debug!("Received OSCQuery path data: {:?}", json);
        Ok(json)
    }

    /// Parse an OSCQuery service from mDNS discovered data
    pub fn parse_service_from_txt_records(&self, service_name: &str, host: &str, port: u16, txt_records: &HashMap<String, String>) -> DiscoveredOscQueryService {
        let mut osc_port = None;

        // Try different possible keys for OSC port
        for key in &["OSC_PORT", "osc.port", "osc.udp.port", "_osc._udp.port"] {
            if let Some(port_str) = txt_records.get(*key) {
                if let Ok(port_num) = port_str.parse::<u16>() {
                    osc_port = Some(port_num);
                    break;
                }
            }
        }

        DiscoveredOscQueryService {
            name: service_name.to_string(),
            host: host.to_string(),
            http_port: port,
            osc_port,
            properties: txt_records.clone(),
        }
    }

    /// Determine if a discovered service is VRChat
    pub fn is_vrchat_service(&self, service: &DiscoveredOscQueryService) -> bool {
        service.name.to_lowercase().contains("vrchat") ||
            service.properties.values().any(|v| v.to_lowercase().contains("vrchat"))
    }
}