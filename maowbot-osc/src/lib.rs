// maowbot-osc/src/lib.rs
use std::net::{UdpSocket, SocketAddr};
use std::sync::Arc;
use tokio::sync::{Mutex, mpsc};
use thiserror::Error;
use tokio::task::JoinHandle;
use crate::oscquery::{OscQueryClient, OscQueryServer};
use crate::vrchat::{discover_vrchat, query_vrchat_oscquery};
use rosc::{OscPacket, OscType};
use tracing::{debug, trace, info, error, warn};
pub mod oscquery;
pub mod vrchat;
pub mod robo; // left as-is
#[derive(Error, Debug)]
pub enum OscError {
    #[error("OSC I/O error: {0}")]
    IoError(String),
    #[error("OSC port conflict or no open port found")]
    PortError,
    #[error("OSCQuery error: {0}")]
    OscQueryError(String),
    #[error("Avatar config parse error: {0}")]
    AvatarConfigError(String),
    #[error("Generic error: {0}")]
    Generic(String),
}
pub type Result<T> = std::result::Result<T, OscError>;
/// Info about VRChat's discovered ports
#[derive(Debug, Clone)]
pub struct VRChatConnectionInfo {
    pub oscquery_host: String,
    pub oscquery_port: u16,
    /// VRChat's **listening** port for OSC (the port we send to),
    /// typically 9000 or random.
    pub osc_send_port: u16,
    /// VRChat's **sending** port for OSC (the port we listen on),
    /// typically 9001 or random.
    pub osc_receive_port: u16,
}
/// A top-level manager that orchestrates the OSC server, VRChat toggles, etc.
pub struct MaowOscManager {
    pub inner: Arc<Mutex<OscManagerInner>>,
    pub oscquery_server: Arc<Mutex<OscQueryServer>>,
    pub vrchat_watcher: Option<Arc<Mutex<crate::vrchat::avatar_watcher::AvatarWatcher>>>,
    pub osc_receiver: Arc<Mutex<Option<OscReceiver>>>,
    pub oscquery_client: Arc<OscQueryClient>,
    pub vrchat_info: Arc<Mutex<Option<VRChatConnectionInfo>>>,
}
pub struct OscManagerInner {
    /// The UDP port on which we are currently listening for OSC
    pub listening_port: Option<u16>,
    pub is_running: bool,
    /// VRChat's UDP port (where we send messages to).
    pub vrchat_osc_port: Option<u16>,
    /// VRChat's TCP port (where we do OSCQuery).
    pub vrchat_oscquery_http_port: Option<u16>,
}
#[derive(Debug)]
pub struct OscManagerStatus {
    pub is_running: bool,
    pub listening_port: Option<u16>,
    pub is_oscquery_running: bool,
    pub oscquery_port: Option<u16>,
    /// In the old code, this was the discovered peers from `mdns-sd`.
    /// We can just return an empty Vec now or remove it entirely.
    pub discovered_peers: Vec<String>,
    pub vrchat_connected: bool,
    pub vrchat_info: Option<VRChatConnectionInfo>,
}
/// Struct to manage receiving OSC messages
pub struct OscReceiver {
    pub receiver_handle: JoinHandle<()>,
    pub incoming_tx: mpsc::UnboundedSender<OscPacket>,
    pub incoming_rx: Option<mpsc::UnboundedReceiver<OscPacket>>,
    shutdown_tx: Option<tokio::sync::watch::Sender<bool>>,

    pub bound_port: u16,
}
impl OscReceiver {
    /// Bind a UDP socket on the given port. If `port == 0`, we bind an ephemeral port.
    /// The actual bound port is extracted from `socket.local_addr()`.
    pub fn new(port: u16) -> Result<Self> {
        let (tx, rx) = mpsc::unbounded_channel();
        let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);

        let bind_addr = SocketAddr::from(([0, 0, 0, 0], port));
        let socket = UdpSocket::bind(bind_addr)
            .map_err(|e| OscError::IoError(format!("Could not bind: {}", e)))?;
        socket
            .set_nonblocking(true)
            .map_err(|e| OscError::IoError(format!("Failed set_nonblocking: {}", e)))?;

        let actual_port = socket
            .local_addr()
            .map_err(|e| OscError::IoError(format!("Could not get local_addr: {}", e)))?
            .port();

        tracing::info!("OSC receiver listening on UDP port {actual_port} (requested {port})");

        let tx_clone = tx.clone();
        let handle = tokio::spawn(async move {
            let mut buf = [0u8; 4096];
            tracing::info!("OSC receiver task is running...");
            let mut shutdown_rx = shutdown_rx;

            loop {
                if *shutdown_rx.borrow() {
                    tracing::info!("OSC receiver got shutdown signal, exiting");
                    break;
                }

                tokio::select! {
                    changed = shutdown_rx.changed() => {
                        if changed.is_ok() && *shutdown_rx.borrow() {
                            tracing::info!("OSC receiver got shutdown signal, exiting");
                            break;
                        }
                    },
                    _ = tokio::time::sleep(tokio::time::Duration::from_millis(10)) => {
                        match socket.recv_from(&mut buf) {
                            Ok((size, addr)) => {
                                match rosc::decoder::decode_udp(&buf[..size]) {
                                    Ok((_remaining, packet)) => {
                                        match &packet {
                                            OscPacket::Message(msg) => {
                                                if !is_common_osc_message(&msg.addr) {
                                                    trace!("OSC Message: {} with {} args from {}", msg.addr, msg.args.len(), addr);
                                                }
                                            },
                                            OscPacket::Bundle(bundle) => {
                                                debug!("OSC Bundle with {} messages from {}", bundle.content.len(), addr);
                                            }
                                        }
                                        let _ = tx_clone.send(packet);
                                    }
                                    Err(e) => {
                                        tracing::error!("OSC decode error: {:?}", e);
                                    }
                                }
                            }
                            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                                // No data
                            }
                            Err(e) => {
                                tracing::error!("OSC receiver error: {:?}", e);
                                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                            }
                        }
                    }
                }
            }

            tracing::info!("OSC receiver task exited cleanly");
        });

        Ok(Self {
            receiver_handle: handle,
            incoming_tx: tx,
            incoming_rx: Some(rx),
            shutdown_tx: Some(shutdown_tx),
            bound_port: actual_port, // Store the real port we got.
        })
    }
    pub fn port(&self) -> u16 {
        self.bound_port
    }
    pub fn take_receiver(&mut self) -> Option<mpsc::UnboundedReceiver<OscPacket>> {
        self.incoming_rx.take()
    }
    pub fn shutdown(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(true);
        }
    }
}
impl MaowOscManager {
    pub fn new() -> Self {
        let inner = OscManagerInner {
            listening_port: None,
            is_running: false,
            vrchat_osc_port: None,
            vrchat_oscquery_http_port: None,
        };
        // Create an OscQueryServer with a placeholder port=0. We'll do ephemeral on .start().
        let oscquery_server = OscQueryServer::new(0);
        let oscquery_client = OscQueryClient::new();
        Self {
            inner: Arc::new(Mutex::new(inner)),
            oscquery_server: Arc::new(Mutex::new(oscquery_server)),
            vrchat_watcher: None,
            osc_receiver: Arc::new(Mutex::new(None)),
            oscquery_client: Arc::new(oscquery_client),
            vrchat_info: Arc::new(Mutex::new(None)),
        }
    }
    /// Return a status snapshot.
    pub async fn get_status(&self) -> Result<OscManagerStatus> {
        let guard = self.inner.lock().await;
        let oscq_guard = self.oscquery_server.lock().await;
        let vrchat_info_guard = self.vrchat_info.lock().await;
        // We removed the old .discovery approach. If you want to show local peers,
        // you can do so using your custom mDNS logic, or just return empty.
        let discovered_peers = Vec::new();
        Ok(OscManagerStatus {
            is_running: guard.is_running,
            listening_port: guard.listening_port,
            is_oscquery_running: oscq_guard.is_running,
            oscquery_port: Some(oscq_guard.http_port),
            discovered_peers,
            vrchat_connected: vrchat_info_guard.is_some(),
            vrchat_info: vrchat_info_guard.clone(),
        })
    }
    /// Start everything:
    /// 1) Discover VRChat's TCP/UDP ports (stub or custom approach)
    /// 2) Create an ephemeral UDP receiver port for our OSC
    /// 3) Create an ephemeral TCP port for our OSCQuery server
    /// 4) Advertise ourselves in mDNS
    pub async fn start_all(&self) -> Result<()> {
        info!("Discovering VRChat services via mDNS...");
        let discovered = discover_vrchat().await?;
        let resolved_info = if let Some(info) = discovered {
            info!(
                "Found VRChat: UDP={} (send), TCP={} (OSCQuery)",
                info.osc_send_port, info.oscquery_port
            );

            // If VRChat's "osc_receive_port" is zero, try /host_info or fallback
            let maybe_port = if info.osc_receive_port == 0 {
                match query_vrchat_oscquery(
                    &self.oscquery_client,
                    &info.oscquery_host,
                    info.oscquery_port,
                    Some(&info.oscquery_host)
                ).await {
                    Ok(Some((_, discovered_rx_port))) => discovered_rx_port,
                    _ => {
                        warn!("We didn't find VRChat's sending port from /host_info. Using default 9001.");
                        9001
                    }
                }
            } else {
                info.osc_receive_port
            };

            VRChatConnectionInfo {
                oscquery_host: info.oscquery_host,
                oscquery_port: info.oscquery_port,
                osc_send_port: info.osc_send_port,
                osc_receive_port: maybe_port,
            }
        } else {
            warn!("VRChat not found via mDNS; using fallback ports (9000,9001).");
            VRChatConnectionInfo {
                oscquery_host: "127.0.0.1".to_string(),
                oscquery_port: 0,
                osc_send_port: 9000,
                osc_receive_port: 9001,
            }
        };

        {
            let mut vrc_guard = self.vrchat_info.lock().await;
            *vrc_guard = Some(resolved_info.clone());
        }

        // 1) Start ephemeral OSC receiver for inbound data from VRChat
        let receiver = OscReceiver::new(0)?; // 0 => ephemeral
        let actual_port = receiver.port();
        {
            let mut lock_inner = self.inner.lock().await;
            lock_inner.listening_port = Some(actual_port);
            lock_inner.is_running = true;
        }
        {
            let mut guard = self.osc_receiver.lock().await;
            *guard = Some(receiver);
            info!("OSC receiver started on ephemeral port {}", actual_port);
        }

        // 2) Start our OSCQuery HTTP server on ephemeral port
        {
            let mut server = self.oscquery_server.lock().await;
            // We now report the same ephemeral port we actually bound:
            server.set_osc_port(actual_port);


            server.start().await?;
            info!(
                "Local OSCQuery server is running on ephemeral port {}",
                server.http_port
            );

            // 3) Advertise ourselves in mDNS
            server.advertise_as_maow().await?;
        }

        Ok(())
    }
    /// Stop watchers, servers, etc.
    pub async fn stop_all(&self) -> Result<()> {
        if let Some(watcher_mutex) = &self.vrchat_watcher {
            let mut watcher = watcher_mutex.lock().await;
            let _ = watcher.stop();
        }
        self.stop_server().await?;
        // Stop the OSCQuery server
        {
            let mut oscq = self.oscquery_server.lock().await;
            if oscq.is_running {
                oscq.stop().await?;
            }
        }
        // Stop the OSC receiver
        {
            let mut osc_rcv = self.osc_receiver.lock().await;
            if let Some(receiver) = osc_rcv.as_mut() {
                receiver.shutdown();
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                receiver.receiver_handle.abort();
                *osc_rcv = None;
                tracing::info!("OSC receiver stopped");
            }
        }
        {
            let mut vrc = self.vrchat_info.lock().await;
            *vrc = None;
        }
        let mut guard = self.inner.lock().await;
        guard.is_running = false;
        guard.listening_port = None;
        guard.vrchat_osc_port = None;
        guard.vrchat_oscquery_http_port = None;
        Ok(())
    }
    /// Currently just returns an empty Vec, since we removed the old discovery code.
    pub async fn discover_local_peers(&self) -> Result<Vec<String>> {
        Ok(vec![])
    }
    pub async fn stop_server(&self) -> Result<()> {
        let mut guard = self.inner.lock().await;
        guard.is_running = false;
        if let Some(p) = guard.listening_port.take() {
            tracing::info!("OSC server on port {p} shut down");
        }
        Ok(())
    }
    /// Send an OSC packet to VRChat’s `osc_send_port`.
    fn send_osc_packet(&self, packet: OscPacket) -> Result<()> {
        // We must send to VRChat’s listen port
        let (dest_port, address) = match self.vrchat_info.try_lock() {
            Ok(guard) => {
                if let Some(v) = guard.as_ref() {
                    (v.osc_send_port, v.oscquery_host.clone())
                } else {
                    (9000, "127.0.0.1".to_string())
                }
            },
            Err(_) => (9000, "127.0.0.1".to_string()),
        };
        let dest_str = format!("{address}:{dest_port}");
        let buf = rosc::encoder::encode(&packet)
            .map_err(|e| OscError::IoError(format!("Encode error: {e:?}")))?;
        let sock = UdpSocket::bind(("127.0.0.1", 0))
            .map_err(|e| OscError::IoError(format!("Bind error: {e}")))?;
        match &packet {
            OscPacket::Message(msg) => {
                tracing::debug!("Sending OSC message: {} to {}", msg.addr, dest_str);
            },
            OscPacket::Bundle(_) => {
                tracing::debug!("Sending OSC bundle to {}", dest_str);
            }
        }
        sock.send_to(&buf, dest_str)
            .map_err(|e| OscError::IoError(format!("Send error: {e}")))?;
        Ok(())
    }
    /// Single-arg helpers
    pub fn send_avatar_parameter_bool(&self, name: &str, value: bool) -> Result<()> {
        let packet = OscPacket::Message(rosc::OscMessage {
            addr: format!("/avatar/parameters/{name}"),
            args: vec![OscType::Bool(value)],
        });
        self.send_osc_packet(packet)
    }
    pub fn send_avatar_parameter_int(&self, name: &str, value: i32) -> Result<()> {
        let packet = OscPacket::Message(rosc::OscMessage {
            addr: format!("/avatar/parameters/{name}"),
            args: vec![OscType::Int(value)],
        });
        self.send_osc_packet(packet)
    }
    pub fn send_avatar_parameter_float(&self, name: &str, value: f32) -> Result<()> {
        let packet = OscPacket::Message(rosc::OscMessage {
            addr: format!("/avatar/parameters/{name}"),
            args: vec![OscType::Float(value)],
        });
        self.send_osc_packet(packet)
    }
    pub async fn send_osc_toggle(&self, param_name: &str, value: f32) -> Result<()> {
        let packet = OscPacket::Message(rosc::OscMessage {
            addr: format!("/avatar/parameters/{param_name}"),
            args: vec![OscType::Float(value)],
        });
        self.send_osc_packet(packet)
    }
    pub fn set_vrchat_watcher(&mut self, watcher: Arc<Mutex<crate::vrchat::avatar_watcher::AvatarWatcher>>) {
        self.vrchat_watcher = Some(watcher);
    }
    pub async fn scan_for_avatars(&self) -> Result<()> {
        if let Some(w) = &self.vrchat_watcher {
            let mut w = w.lock().await;
            w.reload_all_avatars()?;
            Ok(())
        } else {
            Err(OscError::Generic("No VRChat watcher configured".into()))
        }
    }
    pub async fn take_osc_receiver(&self) -> Option<mpsc::UnboundedReceiver<OscPacket>> {
        let mut r = self.osc_receiver.lock().await;
        r.as_mut()?.take_receiver()
    }
}
fn is_common_osc_message(addr: &str) -> bool {
    addr.starts_with("/avatar/parameters/") || addr.starts_with("/tracking/")
}