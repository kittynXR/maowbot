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
use tracing::{error, info, debug, warn};

/// Manages local mDNS advertisement and scanning for `_oscjson._tcp.local.`
pub struct OscQueryDiscovery {
    daemon: Arc<ServiceDaemon>,
    /// We track the *instance names* we have registered, so we can unregister them later.
    registrations: Arc<Mutex<Vec<String>>>,
}

/// Parsed service information including hostname and ports
#[derive(Debug, Clone)]
pub struct DiscoveredService {
    pub name: String,
    pub hostname: String,
    pub addr: Option<String>,
    pub port: u16,
    pub osc_port: Option<u16>,
    pub osc_ip: Option<String>,
}

impl DiscoveredService {
    /// Create from ServiceInfo
    pub fn from_service_info(info: &ServiceInfo) -> Self {
        let fullname = info.get_fullname().to_string();
        let hostname = info.get_hostname().to_string();
        let port = info.get_port(); // This is the HTTP port for OSCQuery

        // Debug TXT records for troubleshooting
        debug!("Service TXT records for {}", fullname);
        // Instead of trying to iterate directly, use the get_property_val_str method to check specific keys
        let txt_keys = ["OSC_PORT", "osc.port", "osc.udp.port", "_osc._udp.port", "OSC_IP", "osc.ip", "OSC.IP"];
        for key in &txt_keys {
            if let Some(value) = info.get_property_val_str(key) {
                debug!("  {} = {}", key, value);
            }
        }

        // Try to find the OSC port from TXT records - try different potential names that VRChat might use
        let osc_port_str = info.get_property_val_str("OSC_PORT")
            .or_else(|| info.get_property_val_str("osc.port"))
            .or_else(|| info.get_property_val_str("osc.udp.port"))
            .or_else(|| info.get_property_val_str("_osc._udp.port"));

        let osc_port = osc_port_str.and_then(|s| s.parse::<u16>().ok());

        // Try to get OSC_IP - try different potential names
        let osc_ip = info.get_property_val_str("OSC_IP")
            .or_else(|| info.get_property_val_str("osc.ip"))
            .or_else(|| info.get_property_val_str("OSC.IP"))
            .map(|s| s.to_string());

        // Try to find the first IP address
        let addr = info.get_addresses().iter().next().map(|ip| ip.to_string());

        Self {
            name: fullname,
            hostname,
            addr,
            port, // This is the HTTP OSCQuery port!
            osc_port,
            osc_ip,
        }
    }

    /// Check if this service is likely to be VRChat
    pub fn is_vrchat(&self) -> bool {
        self.name.to_lowercase().contains("vrchat") ||
            // If the service name doesn't include "vrchat", check the hostname
            self.hostname.to_lowercase().contains("vrchat")
    }
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

    /// Advertise an OSCQuery service with specific properties
    pub async fn advertise_with_properties(&self, service_name: &str, port: u16, properties: HashMap<String, String>) -> Result<()> {
        let service_type = "_oscjson._tcp.local.";
        let instance_name = format!("{}.{}", service_name, service_type);

        // Copy the incoming properties and add any defaults
        let mut all_properties = properties.clone();
        all_properties.insert(String::from("_oscjson._tcp.local.port"), port.to_string());

        let info = ServiceInfo::new(
            service_type,
            &instance_name,
            "maowbot.local.",
            "127.0.0.1",
            port,
            all_properties,
        )
            .map_err(|e| OscError::OscQueryError(format!("ServiceInfo creation error: {e}")))?;

        match self.daemon.register(info) {
            Ok(_) => {
                info!("Advertised mDNS => {instance_name} on port {port} with custom properties");
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

    /// Discover local `_oscjson._tcp.local.` services with more detailed information
    /// Returns detailed information about each discovered service
    pub async fn discover_services(&self) -> Result<Vec<DiscoveredService>> {
        // Try both service types that VRChat uses
        let service_types = ["_oscjson._tcp.local.", "_osc._udp.local."];
        let mut all_discovered = Vec::new();

        for &service_type in &service_types {
            info!("Browsing for mDNS service type: {}", service_type);
            let browser = self
                .daemon
                .browse(service_type)
                .map_err(|e| OscError::OscQueryError(format!("Browse error for {}: {}", service_type, e)))?;

            let (tx, mut rx) = mpsc::channel::<ServiceEvent>(50);

            // We'll capture the browser events in a blocking thread and forward them to `tx`.
            std::thread::spawn(move || {
                while let Ok(event) = browser.recv() {
                    let _ = tx.blocking_send(event);
                }
            });

            let start_time = tokio::time::Instant::now();

            // Increase timeout to 10 seconds for better discovery
            let timeout_duration = Duration::from_secs(10);

            // We'll wait up to timeout_duration total, checking every 0.5s for new events
            while start_time.elapsed() < timeout_duration {
                match timeout(Duration::from_millis(500), rx.recv()).await {
                    Ok(Some(event)) => match event {
                        ServiceEvent::ServiceFound(name_type, ..) => {
                            info!("Service found => {name_type}, resolving...");
                        }
                        ServiceEvent::ServiceRemoved(name_type, ..) => {
                            info!("Service removed => {name_type}");
                        }
                        ServiceEvent::ServiceResolved(info) => {
                            let fullname = info.get_fullname().to_string();

                            // Log all TXT records for debugging
                            debug!("Service resolved: {} on port {}", fullname, info.get_port());
                            debug!("TXT records for {}:", fullname);
                            for key in &["OSC_PORT", "osc.port", "osc.udp.port", "_osc._udp.port",
                                "OSC_IP", "osc.ip", "OSC.IP", "NAME"] {
                                if let Some(value) = info.get_property_val_str(key) {
                                    debug!("  {} = {}", key, value);
                                }
                            }

                            let service = DiscoveredService::from_service_info(&info);
                            info!("Resolved service: {}, hostname: {}, port: {}, OSC port: {:?}",
                                 service.name, service.hostname, service.port, service.osc_port);

                            all_discovered.push(service);
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
                        // timed out after 500ms with no event => keep going until timeout
                    }
                }
            }
        }

        Ok(all_discovered)
    }

    // Update the find_vrchat_service method to be more aggressive
    pub async fn find_vrchat_service(&self) -> Result<Option<DiscoveredService>> {
        let services = self.discover_services().await?;

        info!("Found {} total OSC/OSCQuery services", services.len());
        for (i, svc) in services.iter().enumerate() {
            info!("Service #{}: {} on port {} (OSC port: {:?})",
                 i+1, svc.name, svc.port, svc.osc_port);
        }

        // First, look for services explicitly named VRChat
        for service in &services {
            if service.is_vrchat() {
                info!("Found VRChat OSCQuery service: {:?}", service);
                return Ok(Some(service.clone()));
            }
        }

        // Next, try to find services on 9000/9001 which are VRChat's default ports
        for service in &services {
            if service.port == 9000 || service.port == 9001 ||
                service.osc_port == Some(9000) || service.osc_port == Some(9001) {
                info!("Found potential VRChat OSCQuery service on standard port: {:?}", service);
                return Ok(Some(service.clone()));
            }
        }

        // Last chance - if any service has VRCFT in the name, it's likely the face tracking
        // which is connected to VRChat
        for service in &services {
            if service.name.contains("VRCFT") {
                info!("Found VRCFaceTracking service: {:?}", service);
                return Ok(Some(service.clone()));
            }
        }

        // If we found any service at all, return the first one as last resort
        if !services.is_empty() {
            warn!("No definitive VRChat OSCQuery service found, using first available OSCQuery service");
            return Ok(Some(services[0].clone()));
        }

        info!("No OSCQuery services found at all");
        Ok(None)
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

    // Add to OscQueryDiscovery
    pub async fn debug_all_discovered_services(&self) -> Result<()> {
        info!("Debugging all discoverable mDNS services...");

        // Try all common service types
        let service_types = [
            "_oscjson._tcp.local.",
            "_osc._udp.local.",
            "_http._tcp.local.",
            "_vrchat._tcp.local.", // Maybe VRChat uses a custom type?
        ];

        for &service_type in &service_types {
            info!("Looking for {} services...", service_type);

            let browser = match self.daemon.browse(service_type) {
                Ok(b) => b,
                Err(e) => {
                    warn!("Could not browse {}: {}", service_type, e);
                    continue;
                }
            };

            let (tx, mut rx) = mpsc::channel::<ServiceEvent>(50);

            std::thread::spawn(move || {
                while let Ok(event) = browser.recv() {
                    let _ = tx.blocking_send(event);
                }
            });

            let start_time = tokio::time::Instant::now();
            while start_time.elapsed() < Duration::from_secs(10) {
                match timeout(Duration::from_millis(500), rx.recv()).await {
                    Ok(Some(ServiceEvent::ServiceResolved(info))) => {
                        info!("Found {} service: {}", service_type, info.get_fullname());
                        info!("  Host: {}, Port: {}", info.get_hostname(), info.get_port());
                        info!("  Addresses: {:?}", info.get_addresses());

                        // Log all properties
                        let keys = ["OSC_PORT", "osc.port", "osc.udp.port", "_osc._udp.port",
                            "OSC_IP", "osc.ip", "OSC.IP", "NAME"];
                        for key in &keys {
                            if let Some(value) = info.get_property_val_str(key) {
                                info!("  Property {}: {}", key, value);
                            }
                        }
                    },
                    Ok(Some(_)) => {} // Ignore other events
                    Ok(None) => break, // Channel closed
                    Err(_) => {} // Timeout - keep waiting
                }
            }
        }

        info!("Service debugging complete");
        Ok(())
    }
}