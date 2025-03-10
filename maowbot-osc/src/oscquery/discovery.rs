//! maowbot-osc/src/oscquery/discovery.rs
//!
//! Very simplistic approach to mDNS/Bonjour advertisement and/or discovery
//! for our OSC and OSCQuery services.

use crate::{Result, OscError};
use mdns;
use std::time::Duration;

/// Represents an optional background discovery or advertisement
pub struct OscQueryDiscovery {
    // placeholders
}

impl OscQueryDiscovery {
    pub fn new() -> Self {
        Self {}
    }

    /// Advertise our service over mDNS / Bonjour
    pub async fn advertise(&self, service_name: &str, port: u16) -> Result<()> {
        tracing::info!("Advertising OSC/OSCQuery service '{}' on port {}", service_name, port);

        // Typically you'd do something like:
        // mdns::Responder::spawn(...).unwrap();
        // Then create a service like `_oscjson._tcp.local.` or `_osc._udp.local.`
        // This is left as an exercise; the mdns crate's usage can vary.

        Ok(())
    }

    /// Attempt to discover VRChat or other OSCQuery apps on the local network
    pub async fn discover_peers(&self) -> Result<Vec<String>> {
        let mut found = Vec::new();
        // We can do an mdns::discover::all(...), etc.
        // Example snippet:
        //
        // let stream = mdns::discover::all("_osc._udp.local", Duration::from_secs(5))?
        //     .listen();
        //
        // tokio::pin!(stream);
        // while let Some(Ok(response)) = stream.next().await {
        //     let svc_name = response.service_name();
        //     tracing::info!("Discovered service: {}", svc_name);
        //     found.push(svc_name.to_string());
        // }
        Ok(found)
    }
}
