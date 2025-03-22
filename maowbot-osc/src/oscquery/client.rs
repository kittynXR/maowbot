//! Provide a client for discovering VRChat’s or other apps’ OSC/OSCQuery services locally.
//! Now that we’ve replaced the old mdns-sd approach, we do a quick custom approach.

use crate::{OscError, Result};
use tokio::sync::Mutex;
use std::sync::Arc;
use tracing::{debug, info, warn};

/// Holds discovered VRChat addresses, if any
#[derive(Default)]
pub struct VrchatAddrs {
    pub osc_host: Option<String>,
    pub osc_port: Option<u16>,
    pub oscquery_host: Option<String>,
    pub oscquery_port: Option<u16>,
}

/// Our minimal “client” that attempts to find VRChat’s addresses
pub struct OscQueryClient {
    /// We no longer keep a “discovery” object from the old crate. Instead, if we want to implement
    /// “raw” scanning, we could do so, or we just wait until VRChat queries us.
    pub vrchat_addrs: Arc<Mutex<VrchatAddrs>>,
    pub is_initialized: bool,
}

impl OscQueryClient {
    pub fn new() -> Self {
        Self {
            vrchat_addrs: Arc::new(Mutex::new(VrchatAddrs::default())),
            is_initialized: false,
        }
    }

    pub async fn init(&mut self) -> Result<()> {
        if self.is_initialized {
            return Ok(());
        }
        // No real scanning in this placeholder. If you wanted to passively wait for VRChat queries,
        // you might do so in your server code. Or do your own outbound query to 224.0.0.251.
        self.is_initialized = true;
        info!("OscQueryClient init() – no direct scanning in the new approach");
        Ok(())
    }

    /// Return the host/port for VRChat’s OSC server (where we send data).
    /// If not found, returns an error.
    pub async fn get_vrchat_osc_address(&self) -> Result<(String, u16)> {
        let lock = self.vrchat_addrs.lock().await;
        if let (Some(h), Some(p)) = (lock.osc_host.clone(), lock.osc_port) {
            Ok((h, p))
        } else {
            Err(OscError::OscQueryError("VRChat OSC not found".into()))
        }
    }

    /// Return the host/port for VRChat’s OSCQuery server (where we do HTTP GET).
    /// If not found, returns an error.
    pub async fn get_vrchat_oscquery_address(&self) -> Result<(String, u16)> {
        let lock = self.vrchat_addrs.lock().await;
        if let (Some(h), Some(p)) = (lock.oscquery_host.clone(), lock.oscquery_port) {
            Ok((h, p))
        } else {
            Err(OscError::OscQueryError("VRChat OSCQuery not found".into()))
        }
    }

    /// A placeholder: we do no real refresh. In principle, you could send queries out using
    /// your custom raw mDNS code and parse responses to fill out `vrchat_addrs`.
    pub async fn refresh_vrchat(&self) -> Result<()> {
        warn!("OscQueryClient refresh_vrchat() – not yet implemented with raw mDNS");
        Ok(())
    }
}
