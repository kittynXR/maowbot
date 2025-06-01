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
use tracing::{trace, info, warn, error};
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
    /// 2) Joins the multicast group on each IPv4 interface
    /// 3) Sets the socket nonblocking
    pub fn new() -> Result<Self, OscError> {
        let address = SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), MDNS_PORT);
        let sock2 = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))
            .map_err(|e| OscError::IoError(format!("Failed to create MdnsService socket: {e}")))?;

        sock2
            .set_reuse_address(true)
            .map_err(|e| OscError::IoError(format!("Failed to set SO_REUSEADDR: {e}")))?;

        #[cfg(unix)]
        {
            // On Unix-like systems, also set SO_REUSEPORT
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

        // Join the multicast group on each interface if possible
        #[cfg(not(windows))]
        {
            match if_addrs::get_if_addrs() {
                Ok(ifaces) => {
                    for iface in ifaces {
                        if let IpAddr::V4(ipv4) = iface.ip() {
                            if ipv4.is_loopback() {
                                continue;
                            }
                            let r = socket.join_multicast_v4(&MDNS_MULTICAST_ADDR, &ipv4);
                            if let Err(e) = r {
                                trace!("Failed to join {} on {}: {}", MDNS_MULTICAST_ADDR, ipv4, e);
                            } else {
                                trace!("Joined multicast {} on {}", MDNS_MULTICAST_ADDR, ipv4);
                            }
                        }
                    }
                }
                Err(e) => {
                    warn!("Could not enumerate interfaces for mDNS: {}", e);
                    let _ = socket.join_multicast_v4(&MDNS_MULTICAST_ADDR, &Ipv4Addr::UNSPECIFIED);
                }
            }
        }
        #[cfg(windows)]
        {
            // On Windows, do the "any" approach for IPv4
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

    /// Start responding to inbound queries with our advertised records.
    /// This does not actively “discover” other services; it only replies
    /// to queries. If you want to also discover VRChat, call `start_query_listener()`.
    pub fn start(&mut self) {
        let socket = self.socket.try_clone().expect("Failed to clone UDP socket");
        let adv_map = self.advertised.clone();
        let mut stop_rx = self.stop_tx.subscribe();

        let handle = tokio::spawn(async move {
            let mut buf = [0u8; 4096];
            loop {
                if *stop_rx.borrow() {
                    trace!("mDNS service shutting down");
                    break;
                }
                match socket.recv_from(&mut buf) {
                    Ok((size, from)) => {
                        let data = buf[..size].to_vec();
                        let ascii = String::from_utf8_lossy(&data);
                        trace!("Received packet of size {} from {} ASCII: {}", size, from, ascii);

                        match DnsPacket::parse(crate::oscquery::mdns::dns_reader::DnsReader::new(data)) {
                            Ok(packet) => {
                                if !packet.is_response {
                                    respond_to_queries(&socket, &packet, &adv_map, from);
                                }
                            },
                            Err(e) => {
                                trace!("Failed to parse DNS packet from {}: {}", from, e);
                            }
                        }
                    },
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(10));
                    },
                    Err(e) => {
                        // If WSAEMSGSIZE on Windows
                        if e.raw_os_error() == Some(10040) {
                            warn!("mDNS receive error (oversized datagram 10040): {e}. Skipping packet.");
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
    /// You can call this multiple times for multiple services (e.g. `_osc._udp` & `_oscjson._tcp`).
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

    /// Returns all discovered services that arrived since last time we cleared them.
    pub fn get_discovered(&self) -> Vec<AdvertisedService> {
        let disc = self.discovered.lock().unwrap();
        disc.clone()
    }

    /// Start a listener that can both answer queries and also parse inbound
    /// **responses** from VRChat or other programs. This helps you discover them too.
    pub fn start_query_listener(&mut self) {
        let socket = self.socket.try_clone().expect("Failed to clone UDP socket");
        let adv_map = self.advertised.clone();
        let disc_map = self.discovered.clone();
        let mut stop_rx = self.stop_tx.subscribe();

        let handle = tokio::spawn(async move {
            let mut buf = [0u8; 4096];
            loop {
                if *stop_rx.borrow() {
                    trace!("mDNS query listener shutting down");
                    break;
                }
                match socket.recv_from(&mut buf) {
                    Ok((size, from)) => {
                        let data = buf[..size].to_vec();
                        let ascii = String::from_utf8_lossy(&data);
                        trace!("Packet from {from}, ASCII: {ascii}");
                        match DnsPacket::parse(crate::oscquery::mdns::dns_reader::DnsReader::new(data)) {
                            Ok(packet) => {
                                if packet.is_response {
                                    // This is how we discover VRChat’s announcements
                                    parse_mdns_response(&packet, &disc_map);
                                } else {
                                    // Answer queries for our advertised services
                                    respond_to_queries(&socket, &packet, &adv_map, from);
                                }
                            },
                            Err(e) => {
                                trace!("Failed to parse DNS packet: {e}");
                            }
                        }
                    },
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(5));
                    },
                    Err(e) => {
                        if e.raw_os_error() == Some(10040) {
                            warn!("mDNS receive error (oversized datagram 10040): {e}. Skipping packet.");
                            continue;
                        } else {
                            error!("mDNS receive error: {e}");
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
        {
            let mut disc = self.discovered.lock().unwrap();
            disc.clear();
        }

        let mut packet = DnsPacket::new_response();
        packet.is_response = false; // it's a query
        packet.id = 0;
        packet.questions.push(DnsQuestion {
            labels: labels_from_str(service_type),
            qtype: 0x00FF, // ANY
            qclass: 0x0001,
        });

        let bytes = packet.to_bytes()
            .map_err(|e| OscError::Generic(format!("DnsPacket encoding error: {e}")))?;

        let dest = SocketAddr::new(IpAddr::V4(MDNS_MULTICAST_ADDR), MDNS_PORT);
        let _ = self.socket.send_to(&bytes, dest);

        let start = Instant::now();
        while start.elapsed() < Duration::from_secs(2) {
            std::thread::sleep(Duration::from_millis(50));
        }

        let discovered = {
            let disc = self.discovered.lock().unwrap();
            disc.clone()
        };
        trace!("Discovered {} service(s): {:?}", discovered.len(), discovered);

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

/// Build domain labels from a service string like "_osc._udp.local."
fn labels_from_str(s: &str) -> Vec<String> {
    let clean = s.trim_end_matches('.').to_owned();
    clean.split('.').map(|x| x.to_string()).collect()
}

/// Minimal function to parse resource records from a DNS **response** (used to discover VRChat).
fn parse_mdns_response(packet: &DnsPacket, discovered: &Arc<Mutex<Vec<AdvertisedService>>>) {
    let mut srv_map: HashMap<String, (u16, String)> = HashMap::new();
    let mut a_map: HashMap<String, Ipv4Addr> = HashMap::new();
    let mut has_vrchat_records = false;

    for section in [&packet.answers, &packet.additionals] {
        for ans in section {
            match &ans.rdata {
                RData::PTR(labels) => {
                    let full = labels.join(".");
                    let from = ans.labels.join(".");
                    if full.contains("VRChat-Client") || from.contains("VRChat-Client") {
                        has_vrchat_records = true;
                        trace!("Found VRChat PTR: {} -> {}", from, full);
                    }
                }
                RData::SRV(_, _, port, target_labels) => {
                    let full = ans.labels.join(".");
                    let t_fqdn = target_labels.join(".");
                    if full.contains("VRChat-Client") {
                        has_vrchat_records = true;
                        trace!("Found VRChat SRV => name:{} port:{} target:{}", full, port, t_fqdn);
                    }
                    srv_map.insert(full, (*port, t_fqdn));
                }
                RData::ARecord(ip_bytes) => {
                    if ip_bytes.len() == 4 {
                        let ip = Ipv4Addr::new(ip_bytes[0], ip_bytes[1], ip_bytes[2], ip_bytes[3]);
                        let full = ans.labels.join(".");
                        trace!("Found A => name:{} ip:{}", full, ip);
                        a_map.insert(full, ip);
                    }
                }
                _ => {}
            }
        }
    }

    // Link SRV + A records
    let mut new_entries = Vec::new();
    for (srv_name, (port, target_fqdn)) in srv_map.iter() {
        if !srv_name.contains("VRChat-Client") {
            continue;
        }
        has_vrchat_records = true;

        if let Some(ip) = a_map.get(target_fqdn) {
            let instance_name = extract_instance_name(srv_name);
            let adv = AdvertisedService {
                service_name: instance_name.clone(),
                port: *port,
                address: *ip,
            };
            new_entries.push(adv);
        } else {
            // Attempt partial match
            let mut found = false;
            for (a_name, ip) in &a_map {
                let a_instance = extract_instance_name(a_name);
                let srv_instance = extract_instance_name(srv_name);
                if a_instance == srv_instance || a_name.contains(&srv_instance) || srv_name.contains(&a_instance) {
                    let instance_name = extract_instance_name(srv_name);
                    let adv = AdvertisedService {
                        service_name: instance_name,
                        port: *port,
                        address: *ip,
                    };
                    new_entries.push(adv);
                    found = true;
                    break;
                }
            }
            // Fallback
            if !found && srv_name.contains("VRChat-Client") {
                let instance_name = extract_instance_name(srv_name);
                let adv = AdvertisedService {
                    service_name: instance_name,
                    port: *port,
                    address: Ipv4Addr::new(127,0,0,1),
                };
                new_entries.push(adv);
            }
        }
    }

    // If no direct match but we definitely have VRChat
    if has_vrchat_records && new_entries.is_empty() {
        trace!("Saw VRChat SRV but no A match. Fallback to localhost.");
        for (srv_name, (port, _)) in srv_map.iter() {
            if srv_name.contains("VRChat-Client") {
                let instance_name = extract_instance_name(srv_name);
                let adv = AdvertisedService {
                    service_name: instance_name,
                    port: *port,
                    address: Ipv4Addr::new(127, 0, 0, 1),
                };
                new_entries.push(adv);
            }
        }
    }

    if !new_entries.is_empty() {
        let mut disc = discovered.lock().unwrap();
        disc.extend(new_entries);
    }
}

/// Extract instance name from "MAOW-EA528F._osc._udp.local" -> "MAOW-EA528F"
fn extract_instance_name(full: &str) -> String {
    if let Some(idx) = full.find('.') {
        full[..idx].to_string()
    } else {
        full.to_string()
    }
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
        // We only respond if it's a query for something like _osc._udp.local
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

        let service_type = format!("{}.{}.local.",
                                   q.labels[q.labels.len() - 3],
                                   q.labels[q.labels.len() - 2]);

        for ((inst_name, stype), adv) in locked.iter() {
            if &service_type == stype {
                // Build the FQDN for this instance + service
                // CHANGED: ensure we end with ".osc.local" or ".oscjson.local"
                // instead of ".osc.udp" etc.
                let host_name = make_host_name(inst_name, stype);

                // Example: "MAOW-ABCDEF._osc._udp.local" => instance_fq => ["MAOW-ABCDEF","_osc","_udp","local"]
                let instance_fq = vec![
                    inst_name.clone(),
                    q.labels[q.labels.len() - 3].to_string(),
                    q.labels[q.labels.len() - 2].to_string(),
                    "local".to_string(),
                ];

                // If request wants ANY or PTR, we answer
                if q.qtype == 255 || q.qtype == TYPE_PTR {
                    let ans = DnsResource {
                        labels: q.labels.clone(),
                        rtype: TYPE_PTR,
                        rclass: 0x0001,
                        ttl: 4500,
                        rdata: RData::PTR(instance_fq.clone()),
                    };
                    answers.push(ans);
                }

                // Always add the TXT (which in official VRChat usage just has "txtvers=1")
                let txt = DnsResource {
                    labels: instance_fq.clone(),
                    rtype: TYPE_TXT,
                    rclass: 0x0001,
                    ttl: 4500,
                    rdata: RData::TXT(vec!["txtvers=1".to_string()]),
                };
                additionals.push(txt);

                // Then the SRV record pointing to “host_name”
                let host_labels = host_name.split('.').map(String::from).collect::<Vec<_>>();
                let srv = DnsResource {
                    labels: instance_fq.clone(),
                    rtype: TYPE_SRV,
                    rclass: 0x0001,
                    ttl: 4500,
                    rdata: RData::SRV(0, 0, adv.port, host_labels.clone()),
                };
                additionals.push(srv);

                // Finally an A record for the same “host_name”
                let a = DnsResource {
                    labels: host_labels,
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

// CHANGED: now we unify “{instance_name}.osc.local” or “{instance_name}.oscjson.local”
// instead of “.osc.udp” or “.oscjson.tcp”
fn make_host_name(inst_name: &str, service_type: &str) -> String {
    // Example:
    //   service_type: "_osc._udp.local." => produce  "...osc.local"
    //   service_type: "_oscjson._tcp.local." => produce "...oscjson.local"
    if service_type.contains("_oscjson._tcp") {
        format!("{}.oscjson.local", inst_name)
    } else {
        format!("{}.osc.local", inst_name)
    }
}
