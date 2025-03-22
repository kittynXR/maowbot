// maowbot-osc/src/oscquery/mdns/service.rs
use super::packet::DnsPacket;
use super::records::{
    DnsQuestion, DnsResource, RData,
    TYPE_A, TYPE_PTR, TYPE_SRV, TYPE_TXT,
};
use std::net::{UdpSocket, IpAddr, Ipv4Addr, SocketAddr};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use std::collections::HashMap;
use tokio::task::JoinHandle;
use tokio::sync::watch;
use tracing::{debug, info, warn, error};
use crate::OscError;
use socket2::{Domain, Protocol, Socket, Type};
const MDNS_MULTICAST_ADDR: Ipv4Addr = Ipv4Addr::new(224, 0, 0, 251);
const MDNS_PORT: u16 = 5353;
#[derive(Debug, Clone)]
pub struct AdvertisedService {
    pub service_name: String,
    pub port: u16,
    pub address: Ipv4Addr,
}
/// Our minimal mDNS service that can both advertise our own endpoints
/// and also do queries to discover others (like VRChat).
pub struct MdnsService {
    socket: UdpSocket,
    advertised: Arc<Mutex<HashMap<(String, String), AdvertisedService>>>,
    discovered: Arc<Mutex<Vec<AdvertisedService>>>, // store newly discovered records
    stop_tx: watch::Sender<bool>,
    task_handle: Option<JoinHandle<()>>,
}
impl MdnsService {
    /// Constructor that:
    /// 1) Creates a socket bound to 0.0.0.0:5353
    /// 2) Joins the multicast group on every non-loopback IPv4 interface (UPDATED)
    /// 3) Sets the socket nonblocking
    pub fn new() -> Result<Self, OscError> {
        let address = SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), MDNS_PORT);
        let sock2 = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))
            .map_err(|e| OscError::IoError(format!("Failed to create MdnsService socket: {e}")))?;
        sock2.set_reuse_address(true)
            .map_err(|e| OscError::IoError(format!("Failed to set SO_REUSEADDR: {e}")))?;
        #[cfg(unix)]
        {
            sock2.set_reuse_port(true)
                .map_err(|e| OscError::IoError(format!("Failed to set SO_REUSEPORT: {e}")))?;
        }
        sock2.bind(&address.into())
            .map_err(|e| OscError::IoError(format!("mDNS bind error: {e}")))?;
        // Convert socket2::Socket into std::net::UdpSocket
        let socket = {
            #[cfg(unix)]
            {
                use std::os::unix::io::{IntoRawFd, FromRawFd};
                let raw_fd = sock2.into_raw_fd();
                unsafe { UdpSocket::from_raw_fd(raw_fd) }
            }
            #[cfg(windows)]
            {
                use std::os::windows::io::{IntoRawSocket, FromRawSocket};
                let raw_socket = sock2.into_raw_socket();
                unsafe { UdpSocket::from_raw_socket(raw_socket) }
            }
        };
        // UPDATED: join the multicast group on every IPv4 interface so we
        // can actually receive VRChat's mDNS announcements from 10.x.x.x, etc.
        #[cfg(not(windows))]
        {
            // On Unix-like systems, we can do if_addrs easily:
            match if_addrs::get_if_addrs() {
                Ok(ifaces) => {
                    for iface in ifaces {
                        if let Some(ipv4) = iface.ip().to_owned().to_ipv4() {
                            // Skip loopback, down, or otherwise "odd" interfaces
                            if ipv4.is_loopback() {
                                continue;
                            }
                            let r = socket.join_multicast_v4(&MDNS_MULTICAST_ADDR, &ipv4);
                            if let Err(e) = r {
                                debug!("Failed to join {} on {}: {}", MDNS_MULTICAST_ADDR, ipv4, e);
                            } else {
                                debug!("Joined multicast {} on {}", MDNS_MULTICAST_ADDR, ipv4);
                            }
                        }
                    }
                }
                Err(e) => {
                    warn!("Could not enumerate interfaces for mDNS: {}", e);
                    // Attempt to join on UNSPECIFIED as a fallback
                    let _ = socket.join_multicast_v4(&MDNS_MULTICAST_ADDR, &Ipv4Addr::UNSPECIFIED);
                }
            }
        }
        #[cfg(windows)]
        {
            // On Windows, we typically do join_multicast_v4(..., &Ipv4Addr::UNSPECIFIED)
            // or let user choose an interface index. We'll just do the "any" approach:
            let _ = socket.join_multicast_v4(&MDNS_MULTICAST_ADDR, &Ipv4Addr::UNSPECIFIED);
        }
        socket
            .set_nonblocking(true)
            .map_err(|e| OscError::IoError(format!("set_nonblocking failed: {e}")))?;
        let (stop_tx, _) = watch::channel(false);
        Ok(MdnsService {
            socket,
            advertised: Arc::new(Mutex::new(HashMap::new())),
            discovered: Arc::new(Mutex::new(Vec::new())),
            stop_tx,
            task_handle: None,
        })
    }
    /// Old approach that only responds to inbound queries, ignoring response packets from VRChat.
    /// If you actually need to *discover* VRChat, do `start_query_listener()` instead.
    pub fn start(&mut self) {
        let socket = self.socket.try_clone().expect("Failed to clone UDP socket");
        let adv_map = self.advertised.clone();
        let mut stop_rx = self.stop_tx.subscribe();
        let handle = tokio::spawn(async move {
            // Increased buffer size from 2048 to 4096 to avoid message truncation errors.
            let mut buf = [0u8; 4096];
            loop {
                if *stop_rx.borrow() {
                    debug!("mDNS service shutting down");
                    break;
                }
                match socket.recv_from(&mut buf) {
                    Ok((size, from)) => {
                        let data = buf[..size].to_vec();
                        // Log the raw ASCII text of the received packet.
                        let ascii = String::from_utf8_lossy(&data);
                        debug!("Received packet of size {} from {} with ASCII: {}", size, from, ascii);
                        match DnsPacket::parse(crate::oscquery::mdns::dns_reader::DnsReader::new(data)) {
                            Ok(packet) => {
                                if !packet.is_response {
                                    respond_to_queries(&socket, &packet, &adv_map, from);
                                }
                            },
                            Err(e) => {
                                debug!("Failed to parse DNS packet from {}: {}", from, e);
                            }
                        }
                    },
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(10));
                    },
                    Err(e) => {
                        if e.raw_os_error() == Some(10040) {
                            warn!("mDNS receive error due to oversized datagram (10040): {}. Skipping packet.", e);
                            continue;
                        } else {
                            error!("mDNS receive error: {}", e);
                            std::thread::sleep(Duration::from_secs(1));
                        }
                    }
                }
            }
        });
        self.task_handle = Some(handle);
    }
    /// Stop the background thread and close the socket.
    pub fn stop(&mut self) {
        let _ = self.stop_tx.send(true);
        if let Some(handle) = self.task_handle.take() {
            handle.abort();
        }
        info!("mDNS service stopped");
    }
    /// Advertise an instance, e.g. ("MAOW-ABCDEF", "_osc._udp.local."), with a given port & IP.
    pub fn advertise(
        &self,
        instance_name: &str,
        service_type: &str,
        port: u16,
        address: Ipv4Addr,
    ) {
        let key = (instance_name.to_string(), service_type.to_string());
        let adv = AdvertisedService {
            service_name: instance_name.to_string(),
            port,
            address,
        };
        let mut locked = self.advertised.lock().unwrap();
        locked.insert(key, adv);
        info!("Advertising service: {}.{} at {}:{}", instance_name, service_type, address, port);
    }
    /// Returns all the discovered services that arrived since last time we cleared them.
    pub fn get_discovered(&self) -> Vec<AdvertisedService> {
        let disc = self.discovered.lock().unwrap();
        disc.clone()
    }
    pub fn start_query_listener(&mut self) {
        let socket = self.socket.try_clone().expect("Failed to clone UDP socket");
        let adv_map = self.advertised.clone();
        let disc_map = self.discovered.clone();
        let mut stop_rx = self.stop_tx.subscribe();
        let handle = tokio::spawn(async move {
            // Increased buffer size from 2048 to 4096 bytes.
            let mut buf = [0u8; 4096];
            loop {
                if *stop_rx.borrow() {
                    debug!("mDNS query listener shutting down");
                    break;
                }
                match socket.recv_from(&mut buf) {
                    Ok((size, from)) => {
                        debug!("Received packet of size {} from {}", size, from);
                        let data = buf[..size].to_vec();
                        // Log the raw ASCII text of the received packet.
                        let ascii = String::from_utf8_lossy(&data);
                        debug!("Packet ASCII: {}", ascii);
                        match DnsPacket::parse(crate::oscquery::mdns::dns_reader::DnsReader::new(data)) {
                            Ok(packet) => {
                                if packet.is_response {
                                    // This branch is responsible for picking up VRChat’s announcements
                                    parse_mdns_response(&packet, &disc_map);
                                } else {
                                    // Respond to queries for our advertised services
                                    respond_to_queries(&socket, &packet, &adv_map, from);
                                }
                            },
                            Err(e) => {
                                debug!("Failed to parse DNS packet from {}: {}", from, e);
                            }
                        }
                    },
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(5));
                    },
                    Err(e) => {
                        if e.raw_os_error() == Some(10040) {
                            warn!("mDNS receive error due to oversized datagram (10040): {}. Skipping packet.", e);
                            continue;
                        } else {
                            error!("mDNS receive error: {}", e);
                            std::thread::sleep(Duration::from_millis(50));
                        }
                    }
                }
            }
        });
        self.task_handle = Some(handle);
    }
    /// Send a DNS query for the given service type (e.g. "_osc._udp.local")
    /// optionally filtering by instance substring, then wait for responses.
    pub fn query_for_service(
        &mut self,
        service_type: &str,
        instance_filter: Option<&str>,
    ) -> Result<Vec<AdvertisedService>, OscError> {
        // Clear out old discovered results
        {
            let mut disc = self.discovered.lock().unwrap();
            disc.clear();
        }
        // Build query packet
        let mut packet = DnsPacket::new_response();
        packet.is_response = false; // it's a query
        packet.id = 0;              // for mDNS, ID is typically 0
        packet.questions.push(DnsQuestion {
            labels: labels_from_str(service_type),
            qtype: 0x00FF, // ANY
            qclass: 0x0001,
        });
        let bytes = packet.to_bytes()
            .map_err(|e| OscError::Generic(format!("DnsPacket encoding error: {e}")))?;
        // For mDNS, we typically broadcast to 224.0.0.251:5353
        let dest = SocketAddr::new(IpAddr::V4(MDNS_MULTICAST_ADDR), MDNS_PORT);
        let _ = self.socket.send_to(&bytes, dest);
        // UPDATED: Wait up to 2 seconds for responses instead of 1
        let start = Instant::now();
        while start.elapsed() < Duration::from_secs(2) {
            std::thread::sleep(Duration::from_millis(50));
        }
        // Gather discovered services
        let discovered = {
            let disc = self.discovered.lock().unwrap();
            disc.clone()
        };
        debug!("Discovered {} services: {:?}", discovered.len(), discovered);
        let filtered: Vec<AdvertisedService> = discovered
            .into_iter()
            .filter(|svc| {
                if let Some(filter) = instance_filter {
                    svc.service_name.contains(filter)
                } else {
                    true
                }
            })
            .collect();
        Ok(filtered)
    }
}
/// Build domain name labels from a `_something._udp.local.` style string
fn labels_from_str(s: &str) -> Vec<String> {
    // e.g. "_osc._udp.local." => ["_osc","_udp","local"]
    let clean = s.trim_end_matches('.').to_owned();
    clean.split('.').map(|x| x.to_string()).collect()
}
/// Helper function to extract the bare instance name.
/// For example, if full is "VRChat-Client-9B906A.osc.local", it returns "VRChat-Client-9B906A".
fn extract_instance_name(full: &str) -> String {
    if let Some(idx) = full.find('.') {
        full[..idx].to_string()
    } else {
        full.to_string()
    }
}
/// Minimal function to parse resource records from a DNS *response*
fn parse_mdns_response(packet: &DnsPacket, discovered: &Arc<Mutex<Vec<AdvertisedService>>>) {
    // We'll do a single pass to gather SRV => (port, target FQDN), A => IP, then link them up.
    let mut srv_map: HashMap<String, (u16, String)> = HashMap::new();
    let mut a_map: HashMap<String, Ipv4Addr> = HashMap::new();
    let mut ptr_map: HashMap<String, String> = HashMap::new();
    let mut has_vrchat_records = false;
    // Process both answers and additionals sections
    for section in [&packet.answers, &packet.additionals] {
        for ans in section {
            match &ans.rdata {
                RData::PTR(labels) => {
                    let full = labels.join(".");
                    let from = ans.labels.join(".");
                    // Check if this is a VRChat record
                    if full.contains("VRChat-Client") || from.contains("VRChat-Client") {
                        has_vrchat_records = true;
                        debug!("Found VRChat PTR record: {} -> {}", from, full);
                    }
                    ptr_map.insert(from, full);
                }
                RData::SRV(_, _, port, target_labels) => {
                    let full = ans.labels.join(".");
                    let t_fqdn = target_labels.join(".");
                    if full.contains("VRChat-Client") {
                        has_vrchat_records = true;
                        debug!("Found VRChat SRV record: {} -> port {} -> {}", full, port, t_fqdn);
                    }
                    srv_map.insert(full, (*port, t_fqdn));
                }
                RData::ARecord(ip_bytes) => {
                    if ip_bytes.len() == 4 {
                        let ip = Ipv4Addr::new(ip_bytes[0], ip_bytes[1], ip_bytes[2], ip_bytes[3]);
                        let full = ans.labels.join(".");
                        debug!("Found A record: {} -> {}", full, ip);
                        a_map.insert(full, ip);
                    }
                }
                _ => {}
            }
        }
    }
    // If we have a VRChat SRV record but no matching A record, use the source IP from the
    // mDNS message as a fallback (typical for local VRChat installations)
    let mut new_entries = Vec::new();
    for (srv_name, (port, target_fqdn)) in &srv_map {
        if !srv_name.contains("VRChat-Client") {
            continue;
        }
        has_vrchat_records = true;
        // Get IP from A record if available - using as_str() to fix the type error
        if let Some(ip) = a_map.get(target_fqdn.as_str()) {
            // Extract the bare instance name before any dots.
            let instance_name = extract_instance_name(srv_name);
            let adv = AdvertisedService {
                service_name: instance_name.clone(),
                port: *port,
                address: *ip,
            };
            debug!("Created service entry from A record: {} at {}:{}", instance_name, ip, port);
            new_entries.push(adv);
        } else {
            // Try a more flexible match for the A record
            let mut found = false;
            // Try partial hostname matching - VRChat records sometimes use different formats
            for (a_name, ip) in &a_map {
                // Extract the instance name portion to handle cases like:
                // SRV: VRChat-Client-ABCDEF._osc._udp.local
                // A: VRChat-Client-ABCDEF.osc.local
                let a_instance = extract_instance_name(a_name);
                let srv_instance = extract_instance_name(srv_name);
                if a_instance == srv_instance || a_name.contains(&srv_instance) || srv_name.contains(&a_instance) {
                    let instance_name = extract_instance_name(srv_name);
                    let adv = AdvertisedService {
                        service_name: instance_name.clone(),
                        port: *port,
                        address: *ip,
                    };
                    debug!("Created service entry from partial match: {} at {}:{}", instance_name, ip, port);
                    new_entries.push(adv);
                    found = true;
                    break;
                }
            }
            // If we still haven't found it but have a VRChat record, use a default IP
            if !found && srv_name.contains("VRChat-Client") {
                // Use localhost as a fallback since VRChat is typically on the same machine
                let instance_name = extract_instance_name(srv_name);
                let adv = AdvertisedService {
                    service_name: instance_name.clone(),
                    port: *port,
                    address: Ipv4Addr::new(127, 0, 0, 1),
                };
                debug!("Created service entry with fallback IP: {} at 127.0.0.1:{}", instance_name, port);
                new_entries.push(adv);
            }
        }
    }
    // If we saw VRChat records but couldn't create service entries yet, try harder
    if has_vrchat_records && new_entries.is_empty() {
        debug!("Saw VRChat records but couldn't match A records properly; creating entries from SRV only");
        for (srv_name, (port, _)) in &srv_map {
            if srv_name.contains("VRChat-Client") {
                let instance_name = extract_instance_name(srv_name);
                let adv = AdvertisedService {
                    service_name: instance_name.clone(),
                    port: *port,
                    address: Ipv4Addr::new(127, 0, 0, 1), // Use localhost since that's typical for VRChat
                };
                debug!("Created fallback service entry: {} on 127.0.0.1:{}", instance_name, port);
                new_entries.push(adv);
            }
        }
    }
    if !new_entries.is_empty() {
        let mut disc = discovered.lock().unwrap();
        disc.extend(new_entries);
    }
}
/// If a service name ends with `.local`, strip that off. Also remove trailing dot.
fn trim_local_dot(s: &str) -> String {
    s.trim_end_matches(".local").trim_end_matches('.').to_string()
}
/// Respond to queries for *our* advertised services
fn respond_to_queries(
    socket: &UdpSocket,
    packet: &DnsPacket,
    adv_map: &Arc<Mutex<HashMap<(String, String), AdvertisedService>>>,
    _remote: SocketAddr,
) {
    let mut answers = Vec::new();
    let mut additionals = Vec::new();
    let locked = adv_map.lock().unwrap();
    for q in &packet.questions {
        if q.labels.len() < 3 {
            continue;
        }
        if q.labels[q.labels.len() - 1] != "local" {
            continue;
        }
        let is_osc_udp = q.labels[q.labels.len() - 2] == "_udp"
            && q.labels[q.labels.len() - 3].starts_with("_osc");
        let is_oscjson_tcp = q.labels[q.labels.len() - 2] == "_tcp"
            && q.labels[q.labels.len() - 3].starts_with("_oscjson");
        if !(is_osc_udp || is_oscjson_tcp) {
            continue;
        }
        let service_type = format!("{}.{}.local.", q.labels[q.labels.len() - 3], q.labels[q.labels.len() - 2]);
        for ((inst_name, stype), adv) in locked.iter() {
            if stype == &service_type {
                let instance_fq = vec![
                    inst_name.clone(),
                    q.labels[q.labels.len() - 3].to_string(),
                    q.labels[q.labels.len() - 2].to_string(),
                    "local".to_string()
                ];
                if q.qtype == 255 || q.qtype == TYPE_PTR {
                    let ans = DnsResource {
                        labels: q.labels.clone(),
                        rtype: TYPE_PTR,
                        rclass: 0x0001,
                        ttl: 120,
                        rdata: RData::PTR(instance_fq.clone()),
                    };
                    answers.push(ans);
                }
                // Build the TXT record first.
                let txt = DnsResource {
                    labels: instance_fq.clone(),
                    rtype: TYPE_TXT,
                    rclass: 0x0001,
                    ttl: 120,
                    rdata: RData::TXT(vec!["txtvers=1".to_string()]),
                };
                additionals.push(txt);
                // Then build the SRV record.
                let host_name = make_host_name(inst_name, stype);
                let srv = DnsResource {
                    labels: instance_fq.clone(),
                    rtype: TYPE_SRV,
                    rclass: 0x0001,
                    ttl: 120,
                    rdata: RData::SRV(0, 0, adv.port, vec![host_name.clone()]),
                };
                additionals.push(srv);
                // Finally add the A record.
                let a = DnsResource {
                    labels: vec![host_name.clone()],
                    rtype: TYPE_A,
                    rclass: 0x0001,
                    ttl: 120,
                    rdata: RData::ARecord(vec![
                        adv.address.octets()[0],
                        adv.address.octets()[1],
                        adv.address.octets()[2],
                        adv.address.octets()[3],
                    ]),
                };
                additionals.push(a);
            }
        }
    }
    if answers.is_empty() && additionals.is_empty() {
        return;
    }
    let mut resp = DnsPacket::new_response();
    resp.answers = answers;
    resp.additionals = additionals;
    match resp.to_bytes() {
        Ok(bytes) => {
            let dest = SocketAddr::new(IpAddr::V4(MDNS_MULTICAST_ADDR), MDNS_PORT);
            let _ = socket.send_to(&bytes, dest);
        }
        Err(e) => {
            error!("Failed to encode mDNS response: {}", e);
        }
    }
}
/// New helper to construct a proper host name for the advertised service.
/// For _osc._udp services, returns "{instance}.osc", and for _oscjson._tcp returns "{instance}.oscjson.tcp".
fn make_host_name(inst_name: &str, service_type: &str) -> String {
    if service_type.starts_with("_oscjson") {
        format!("{}.oscjson.tcp", inst_name)
    } else if service_type.starts_with("_osc") {
        // For UDP, append ".osc.udp" so the SRV and A records become "{instance}.osc.udp"
        format!("{}.osc.udp", inst_name)
    } else {
        format!("{}.local", inst_name)
    }
}
