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
use tracing::{error, info, debug};

/// Manages local mDNS advertisement and scanning for `_oscjson._tcp.local.`
pub struct OscQueryDiscovery {
    daemon: Arc<ServiceDaemon>,
    /// We track the *instance names* we have registered, so we can unregister them later.
    registrations: Arc<Mutex<Vec<String>>>,
}

/// Parsed service information including hostname and ports
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
        let port = info.get_port();

        // Try to find the OSC port from TXT records
        let osc_port_str = info.get_property_val_str("OSC_PORT")
            .or_else(|| info.get_property_val_str("osc.port"))
            .or_else(|| info.get_property_val_str("osc.udp.port"))
            .or_else(|| info.get_property_val_str("_osc._udp.port"));

        let osc_port = osc_port_str.and_then(|s| s.parse::<u16>().ok());

        // Try to get OSC_IP
        let osc_ip = info.get_property_val_str("OSC_IP")
            .or_else(|| info.get_property_val_str("osc.ip"))
            .map(|s| s.to_string());

        // Try to find the first IP address
        let addr = info.get_addresses().iter().next().map(|ip| ip.to_string());

        Self {
            name: fullname,
            hostname,
            addr,
            port,
            osc_port,
            osc_ip,
        }
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
                    // Inside the discover_services method where we handle the ServiceResolved event
                    ServiceEvent::ServiceResolved(info) => {
                        let service = DiscoveredService::from_service_info(&info);

                        debug!("Resolved service: {}, hostname: {}, port: {}, OSC port: {:?}",
                        service.name, service.hostname, service.port, service.osc_port);

                        discovered.push(service);
                    }
                    _ => {} // Ignore other events
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

    /// Find VRChat OSCQuery service
    pub async fn find_vrchat_service(&self) -> Result<Option<DiscoveredService>> {
        let services = self.discover_services().await?;

        // Look for VRChat's service
        for service in services {
            // Check if this looks like a VRChat service
            if service.name.to_lowercase().contains("vrchat") {
                debug!("Found VRChat service: {:?}", service);
                return Ok(Some(service));
            }
        }

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
}