//! maowbot-osc/src/oscquery/discovery.rs
//!
//! Implements mDNS advertisement and service discovery using `mdns-sd` ^0.13.3
//! specifically for `_oscjson._tcp.local.` services (OSCQuery).

use crate::{Result, OscError};
use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use tokio::time::{timeout, Duration};
use tracing::{error, info, warn};

/// Manages local mDNS advertisement and scanning for `_oscjson._tcp.local.`
pub struct OscQueryDiscovery {
    daemon: Arc<ServiceDaemon>,
    /// We track the *instance names* we have registered, so we can unregister them later.
    registrations: Arc<Mutex<Vec<String>>>,
}

impl OscQueryDiscovery {
    /// Create a new `OscQueryDiscovery` using `mdns-sd`'s `ServiceDaemon`.
    pub fn new() -> Result<Self> {
        let daemon = ServiceDaemon::new()
            .map_err(|e| OscError::OscQueryError(format!("Failed to create mDNS daemon: {e}")))?;
        Ok(Self {
            daemon: Arc::new(daemon),
            registrations: Arc::new(Mutex::new(Vec::new())),
        })
    }

    /// Advertise an OSCQuery service `_oscjson._tcp.local.` via mDNS.
    ///
    /// # Parameters
    /// * `service_name`: a user-friendly instance name (e.g. "MaowBotOSCQuery").
    /// * `port`: The TCP port (the HTTP server for OSCQuery).
    ///
    /// In `mdns-sd` 0.13.3, `ServiceInfo::new` expects:
    /// `new(service_type, service_name, ip, port, properties)`.
    /// We use `"maowbot.local."` as a placeholder hostname that implements `AsIpAddrs`.
    pub async fn advertise(&self, service_name: &str, port: u16) -> Result<()> {
        let service_type = "_oscjson._tcp.local.";
        let instance_name = format!("{}.{}", service_name, service_type);

        // If you want extra TXT properties, add them here:
        let mut properties = HashMap::<String, String>::new();
        properties.insert(String::from("_oscjson._tcp.local.port"), port.to_string());
        // 5-arg version for mdns-sd 0.13.3
        //   1) service_type
        //   2) instance_name
        //   3) ip/host => must implement AsIpAddrs ("maowbot.local." is just a placeholder)
        //   4) port
        //   5) properties => must implement IntoTxtProperties (HashMap<String, String> is fine)
        let info = ServiceInfo::new(
            service_type,
            &instance_name,
            "maowbot.local.", // or "127.0.0.1" if you prefer
            "127.0.0.1",
            port,
            properties,
        )
            .map_err(|e| OscError::OscQueryError(format!("ServiceInfo creation error: {e}")))?;

        match self.daemon.register(info) {
            Ok(_) => {
                info!("Advertised mDNS => {instance_name} on port {port}");
                let mut reg = self.registrations.lock().unwrap();
                reg.push(instance_name);
                Ok(())
            }
            Err(e) => Err(OscError::OscQueryError(format!(
                "Failed registering mDNS: {e}"
            ))),
        }
    }

    /// Discover local `_oscjson._tcp.local.` services for up to 5 seconds.
    /// Returns a list of **fully resolved** service names (the "fullname").
    pub async fn discover_peers(&self) -> Result<Vec<String>> {
        let service_type = "_oscjson._tcp.local.";
        let browser = self
            .daemon
            .browse(service_type)
            .map_err(|e| OscError::OscQueryError(format!("Browse error: {e}")))?;

        let (tx, mut rx) = mpsc::channel::<ServiceEvent>(50);

        // We'll capture the browser events in a blocking thread and forward them to `tx`.
        std::thread::spawn(move || {
            while let Ok(event) = browser.recv() {
                let _ = tx.blocking_send(event);
            }
        });

        let start_time = tokio::time::Instant::now();
        let mut discovered = Vec::new();

        // We'll wait up to 5 seconds total, checking every 0.5s for new events
        while start_time.elapsed() < Duration::from_secs(5) {
            match timeout(Duration::from_millis(500), rx.recv()).await {
                Ok(Some(event)) => match event {
                    ServiceEvent::ServiceFound(name_type, ..) => {
                        info!("Service found => {name_type}, still resolving...");
                    }
                    ServiceEvent::ServiceRemoved(name_type, ..) => {
                        info!("Service removed => {name_type}");
                    }
                    ServiceEvent::ServiceResolved(info) => {
                        let fullname = info.get_fullname().to_string();
                        info!("Resolved service => {fullname}, port={}", info.get_port());
                        discovered.push(fullname);
                    }
                    ServiceEvent::SearchStarted(_ty) => {
                        info!("mDNS Search started for {service_type}");
                    }
                    ServiceEvent::SearchStopped(_ty) => {
                        info!("mDNS Search stopped for {service_type}");
                    }
                },
                Ok(None) => {
                    // Sender closed, no more events
                    break;
                }
                Err(_) => {
                    // timed out after 500ms with no event => keep going until 5s total
                }
            }
        }

        Ok(discovered)
    }

    /// Unregister all previously advertised services, stopping their mDNS announcements.
    pub fn stop(&self) -> Result<()> {
        let mut reg_lock = self.registrations.lock().unwrap();
        for instance_name in reg_lock.drain(..) {
            if let Err(e) = self.daemon.unregister(&instance_name) {
                error!("Failed to unregister {instance_name}: {e}");
            } else {
                info!("Unregistered mDNS service: {instance_name}");
            }
        }
        Ok(())
    }
}
