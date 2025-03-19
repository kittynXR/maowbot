pub mod oscquery;
pub mod vrchat;
pub mod robo;

use std::net::{UdpSocket, SocketAddr};
use std::sync::Arc;
use tokio::sync::{Mutex, mpsc};
use thiserror::Error;
use tokio::task::JoinHandle;

use crate::oscquery::{OscQueryServer, OscQueryClient};
use crate::vrchat::{query_vrchat_oscquery, discover_vrchat};
use rosc::{OscPacket, OscType};
use tracing::{debug, trace, info, error, warn};

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

/// A top-level manager that might orchestrate the entire OSC server and
/// all VRChat toggles/robotic logic.
///
/// For instance, it can:
/// - Spin up an OSC client
/// - Connect to VRChat's OSCQuery server
/// - Parse VRChat's avatar JSON
/// - Maintain a list of toggles
/// - Send toggles & chatbox messages to VRChat
///
/// This is an example integrated approach.
pub struct MaowOscManager {
    pub inner: Arc<Mutex<OscManagerInner>>,
    pub oscquery_server: Arc<Mutex<OscQueryServer>>, // Used only for advertising our app, not necessary
    pub vrchat_watcher: Option<Arc<Mutex<vrchat::avatar_watcher::AvatarWatcher>>>,
    pub osc_receiver: Arc<Mutex<Option<OscReceiver>>>,
    pub oscquery_client: Arc<OscQueryClient>,
    pub vrchat_info: Arc<Mutex<Option<VRChatConnectionInfo>>>,
}

/// Information about VRChat's OSC/OSCQuery endpoints
#[derive(Debug, Clone)]
pub struct VRChatConnectionInfo {
    pub oscquery_host: String,
    pub oscquery_port: u16,
    pub osc_send_port: u16,     // VRChat listens on 9000 by default
    pub osc_receive_port: u16,  // VRChat sends to 9001 by default
}

pub struct OscManagerInner {
    pub listening_port: Option<u16>,
    pub is_running: bool,
    pub vrchat_osc_port: Option<u16>,  // Port where VRChat is sending OSC data
    pub vrchat_oscquery_http_port: Option<u16>,  // Port where VRChat's OSCQuery is running
}

#[derive(Debug)]
pub struct OscManagerStatus {
    pub is_running: bool,
    pub listening_port: Option<u16>,
    pub is_oscquery_running: bool,
    pub oscquery_port: Option<u16>,
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
}

impl OscReceiver {
    pub fn new(port: u16) -> Result<Self> {
        let (tx, rx) = mpsc::unbounded_channel();
        let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);

        // Create the socket for listening - bind to all interfaces
        let socket_addr = SocketAddr::from(([0, 0, 0, 0], port));
        tracing::info!("Binding OSC receiver socket to {}", socket_addr);

        let socket = match UdpSocket::bind(socket_addr) {
            Ok(s) => s,
            Err(e) => {
                tracing::error!("Failed to bind to {}: {}", socket_addr, e);
                // Try alternate binding
                let local_addr = SocketAddr::from(([127, 0, 0, 1], port));
                tracing::info!("Trying alternate binding to {}", local_addr);
                UdpSocket::bind(local_addr)
                    .map_err(|e2| OscError::IoError(format!("Could not bind to any address: {}, then {}", e, e2)))?
            }
        };

        socket.set_nonblocking(true)
            .map_err(|e| OscError::IoError(format!("Failed to set nonblocking: {}", e)))?;

        // Move ownership of the socket to the spawned task
        let tx_clone = tx.clone();
        let handle = tokio::spawn(async move {
            // Increase buffer size for larger OSC packets
            let mut buf = [0u8; 4096]; // Increased from 1024
            tracing::info!("OSC receiver listening on UDP port {}...", port);

            let mut shutdown_rx = shutdown_rx;

            loop {
                // Check for shutdown signal
                if *shutdown_rx.borrow() {
                    tracing::info!("OSC receiver received shutdown signal, exiting");
                    break;
                }

                // Non-blocking processing with a small delay
                tokio::select! {
                    // Check for shutdown signal change
                    result = shutdown_rx.changed() => {
                        if result.is_ok() && *shutdown_rx.borrow() {
                            tracing::info!("OSC receiver received shutdown signal, exiting");
                            break;
                        }
                    }

                    // Small delay for non-blocking
                    _ = tokio::time::sleep(tokio::time::Duration::from_millis(10)) => {
                        // Non-blocking receive
                        match socket.recv_from(&mut buf) {
                            Ok((size, addr)) => {
                                // tracing::debug!("Received OSC packet: {} bytes from {}", size, addr);
                                // Parse packet
                                match rosc::decoder::decode_udp(&buf[..size]) {
                                    Ok((_remaining, packet)) => {
                                        // Log received packet for debugging
                                        match &packet {
                                            OscPacket::Message(msg) => {
                                                if !is_common_osc_message(&msg.addr) {
                                                    trace!("OSC Message: {} with {} args", msg.addr, msg.args.len());
                                                }
                                            },
                                            OscPacket::Bundle(bundle) => {
                                                debug!("OSC Bundle with {} messages", bundle.content.len());
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
                                // No data available, continue
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
        })
    }

    pub fn take_receiver(&mut self) -> Option<mpsc::UnboundedReceiver<OscPacket>> {
        self.incoming_rx.take()
    }

    pub fn shutdown(&mut self) {
        // Send shutdown signal if we have a sender
        if let Some(tx) = self.shutdown_tx.take() {
            if let Err(e) = tx.send(true) {
                tracing::error!("Failed to send shutdown signal to OSC receiver: {:?}", e);
            } else {
                tracing::info!("Sent shutdown signal to OSC receiver task");
            }
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
        // Create OSCQuery server on port 8080 for HTTP
        let oscquery_server = OscQueryServer::new(8080);
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

    pub async fn get_status(&self) -> Result<OscManagerStatus> {
        // Lock the inner struct here:
        let guard = self.inner.lock().await;
        let oscq_guard = self.oscquery_server.lock().await;
        let vrchat_info_guard = self.vrchat_info.lock().await;

        // Get discovered peers if available
        let discovered_peers = if let Some(discovery) = &oscq_guard.discovery {
            match discovery.discover_peers().await {
                Ok(peers) => peers,
                Err(_) => vec![]
            }
        } else {
            vec![]
        };

        // We build a simple status object:
        let status = OscManagerStatus {
            is_running: guard.is_running,
            listening_port: guard.listening_port,
            is_oscquery_running: oscq_guard.is_running,
            oscquery_port: Some(oscq_guard.http_port),
            discovered_peers,
            vrchat_connected: vrchat_info_guard.is_some(),
            vrchat_info: vrchat_info_guard.clone(),
        };

        Ok(status)
    }

    /// Starts the OSC client and connection to VRChat
    /// This is the preferred way to start all OSC functionality
    pub async fn start_all(&self) -> Result<()> {
        // 1) Start by discovering VRChat's OSCQuery service
        info!("Discovering VRChat OSCQuery service...");
        let vrchat_service = discover_vrchat().await?;

        if vrchat_service.is_none() {
            warn!("VRChat OSCQuery service not found. Is VRChat running with OSC enabled?");
            warn!("Will try to use default VRChat ports (9000/9001)");

            // Set default VRChat connection info
            let default_info = VRChatConnectionInfo {
                oscquery_host: "127.0.0.1".to_string(),
                oscquery_port: 0, // We don't know the OSCQuery port
                osc_send_port: 9000, // VRChat listens here by default
                osc_receive_port: 9001, // VRChat sends here by default
            };

            {
                let mut info_guard = self.vrchat_info.lock().await;
                *info_guard = Some(default_info.clone());
            }

            // We'll directly listen on port 9001 to receive data from VRChat
            {
                let mut inner = self.inner.lock().await;
                inner.listening_port = Some(9001);
                inner.is_running = true;
                inner.vrchat_osc_port = Some(9001); // VRChat's OSC output port
            }
        } else {
            // We found VRChat's OSCQuery service
            let service = vrchat_service.unwrap();
            info!("Found VRChat OSCQuery service at {}:{}",
                service.hostname, service.port);

            // Query VRChat's OSCQuery to get its OSC port info
            let query_result = query_vrchat_oscquery(&self.oscquery_client,
                                                     service.addr.as_deref().unwrap_or(&service.hostname),
                                                     service.port).await?;

            if let Some((ip, port)) = query_result {
                info!("VRChat is sending OSC data to {}:{}", ip, port);

                // Store VRChat connection info
                let vrchat_info = VRChatConnectionInfo {
                    oscquery_host: service.hostname.clone(),
                    oscquery_port: service.port,
                    osc_send_port: 9000, // VRChat always listens on 9000
                    osc_receive_port: port, // The port VRChat sends to
                };

                {
                    let mut info_guard = self.vrchat_info.lock().await;
                    *info_guard = Some(vrchat_info);
                }

                // Update our inner state - we'll listen directly on the port VRChat sends to
                {
                    let mut inner = self.inner.lock().await;
                    inner.listening_port = Some(port);
                    inner.is_running = true;
                    inner.vrchat_osc_port = Some(port);
                    inner.vrchat_oscquery_http_port = Some(service.port);
                }
            } else {
                warn!("Found VRChat's OSCQuery service but couldn't get OSC port info");
                warn!("Will try to use default VRChat ports (9000/9001)");

                // Set default VRChat connection info
                let default_info = VRChatConnectionInfo {
                    oscquery_host: service.hostname.clone(),
                    oscquery_port: service.port,
                    osc_send_port: 9000, // VRChat listens here by default
                    osc_receive_port: 9001, // VRChat sends here by default
                };

                {
                    let mut info_guard = self.vrchat_info.lock().await;
                    *info_guard = Some(default_info);
                }

                // Update our inner state - use default ports
                {
                    let mut inner = self.inner.lock().await;
                    inner.listening_port = Some(9001);
                    inner.is_running = true;
                    inner.vrchat_osc_port = Some(9001);
                    inner.vrchat_oscquery_http_port = Some(service.port);
                }
            }
        }

        // Get our chosen listening port
        let chosen_port;
        {
            let inner = self.inner.lock().await;
            chosen_port = inner.listening_port.unwrap();
        }

        // 3) Start the OSC receiver to listen for messages from VRChat
        {
            let mut osc_rcv = self.osc_receiver.lock().await;
            if osc_rcv.is_none() {
                match OscReceiver::new(chosen_port) {
                    Ok(receiver) => {
                        *osc_rcv = Some(receiver);
                        info!("OSC receiver started on port {}", chosen_port);
                    }
                    Err(e) => {
                        error!("Failed to start OSC receiver on port {}: {:?}", chosen_port, e);
                        return Err(e);
                    }
                }
            }
        }

        // 4) Start the avatar watcher if configured
        if let Some(watcher_mutex) = &self.vrchat_watcher {
            let mut watcher = watcher_mutex.lock().await;
            if let Err(e) = watcher.start() {
                error!("Failed to start VRChat avatar watcher: {:?}", e);
            } else {
                info!("VRChat avatar watcher started");
            }
        }

        // 5) Start our own OSCQuery server to advertise ourselves (helps with auto-discovery)
        // 5) Start our own OSCQuery server to advertise ourselves (helps with auto-discovery)
        {
            let mut oscq = self.oscquery_server.lock().await;
            oscq.set_osc_port(chosen_port);
            if let Err(e) = oscq.start().await {
                warn!("Failed to start OSCQuery server: {:?}", e);
            } else {
                info!("OSCQuery server started on port {} for HTTP and port {} for OSC",
                     oscq.http_port, chosen_port);
            }
        }

        Ok(())
    }

    /// Stop watching for file changes and clean up resources
    pub async fn stop_all(&self) -> Result<()> {
        // Stop the VRChat avatar watcher first if available
        if let Some(watcher_mutex) = &self.vrchat_watcher {
            let mut watcher = watcher_mutex.lock().await;
            if let Err(e) = watcher.stop() {
                tracing::error!("Failed to stop VRChat avatar watcher: {:?}", e);
            } else {
                tracing::info!("VRChat avatar watcher stopped");
            }
        }

        // Stop the UDP OSC
        self.stop_server().await?;

        // Stop the OSCQuery server
        {
            let mut oscq = self.oscquery_server.lock().await;
            if oscq.is_running {
                oscq.stop().await?;
            }
        }

        // Stop the OSC receiver if running
        {
            let mut osc_rcv = self.osc_receiver.lock().await;
            if let Some(receiver) = osc_rcv.as_mut() {
                // Send shutdown signal first
                receiver.shutdown();

                // Give the task a moment to shut down gracefully
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

                // Now abort the task to ensure it's gone (belt and suspenders approach)
                receiver.receiver_handle.abort();

                // Remove the receiver
                *osc_rcv = None;
                tracing::info!("OSC receiver stopped");
            }
        }

        // Clear VRChat info
        {
            let mut vrchat_info = self.vrchat_info.lock().await;
            *vrchat_info = None;
        }

        Ok(())
    }

    /// (Optional) Manually discover local peers. Usually we spawn this in background
    /// so it won't block server startup.
    pub async fn discover_local_peers(&self) -> Result<Vec<String>> {
        let oscq = self.oscquery_server.lock().await;
        if let Some(discovery) = &oscq.discovery {
            let discovered = discovery.discover_peers().await?;
            for svc_name in &discovered {
                tracing::info!("Found local OSCQuery service => {svc_name}");
            }
            Ok(discovered)
        } else {
            // If there's no discovery object, return an empty list or error
            Ok(vec![])
        }
    }

    /// Wait for the server to stop.
    pub async fn stop_server(&self) -> Result<()> {
        let mut guard = self.inner.lock().await;
        guard.is_running = false;
        if let Some(p) = guard.listening_port.take() {
            tracing::info!("OSC server on port {p} has been shut down.");
        }
        Ok(())
    }

    // ------------------------------------------------------------------------
    // Common helper for sending a raw OSC packet to VRChat (which listens on 9000 by default).

    fn send_osc_packet(&self, packet: OscPacket) -> Result<()> {
        // Get VRChat's OSC input port from our stored info, or use default 9000
        let port = match self.vrchat_info.try_lock() {
            Ok(guard) => match &*guard {
                Some(info) => info.osc_send_port,
                None => 9000 // Default if no info
            },
            Err(_) => 9000 // Default if lock fails
        };

        let address = format!("127.0.0.1:{}", port);
        let buf = rosc::encoder::encode(&packet)
            .map_err(|e| OscError::IoError(format!("Encode error: {e:?}")))?;

        let sock = UdpSocket::bind(("127.0.0.1", 0))
            .map_err(|e| OscError::IoError(format!("Bind sock error: {e}")))?;

        // Log what we're sending
        match &packet {
            OscPacket::Message(msg) => {
                tracing::debug!("Sending OSC message: {} with {} args to {}",
                               msg.addr, msg.args.len(), address);
            },
            OscPacket::Bundle(_) => {
                tracing::debug!("Sending OSC bundle to {}", address);
            }
        }

        sock.send_to(&buf, address)
            .map_err(|e| OscError::IoError(format!("Send error: {e}")))?;
        Ok(())
    }

    // ------------------------------------------------------------------------
    // Single-argument sending methods for avatar parameters (bool, int, float).

    /// Send a boolean value to an avatar parameter:
    /// address => `/avatar/parameters/<param_name>`, type => bool
    pub fn send_avatar_parameter_bool(&self, param_name: &str, value: bool) -> Result<()> {
        let packet = OscPacket::Message(rosc::OscMessage {
            addr: format!("/avatar/parameters/{param_name}"),
            args: vec![OscType::Bool(value)],
        });
        self.send_osc_packet(packet)
    }

    /// Send an integer value to an avatar parameter:
    /// address => `/avatar/parameters/<param_name>`, type => int
    pub fn send_avatar_parameter_int(&self, param_name: &str, value: i32) -> Result<()> {
        let packet = OscPacket::Message(rosc::OscMessage {
            addr: format!("/avatar/parameters/{param_name}"),
            args: vec![OscType::Int(value)],
        });
        self.send_osc_packet(packet)
    }

    /// Send a float value to an avatar parameter:
    /// address => `/avatar/parameters/<param_name>`, type => float
    pub fn send_avatar_parameter_float(&self, param_name: &str, value: f32) -> Result<()> {
        let packet = OscPacket::Message(rosc::OscMessage {
            addr: format!("/avatar/parameters/{param_name}"),
            args: vec![OscType::Float(value)],
        });
        self.send_osc_packet(packet)
    }

    /// A legacy example that sends a float "toggle" to a parameter. For many toggles, you
    /// might want to send 0.0 or 1.0 as a float. Prefer the typed methods above.
    pub async fn send_osc_toggle(&self, param_name: &str, value: f32) -> Result<()> {
        let packet = OscPacket::Message(rosc::OscMessage {
            addr: format!("/avatar/parameters/{}", param_name),
            args: vec![OscType::Float(value)],
        });
        self.send_osc_packet(packet)
    }

    /// Set the VRChat avatar watcher
    pub fn set_vrchat_watcher(&mut self, watcher: Arc<Mutex<vrchat::avatar_watcher::AvatarWatcher>>) {
        self.vrchat_watcher = Some(watcher);
    }

    pub async fn scan_for_avatars(&self) -> Result<()> {
        if let Some(watcher_mutex) = &self.vrchat_watcher {
            let mut watcher = watcher_mutex.lock().await;
            match watcher.reload_all_avatars() {
                Ok(_) => {
                    tracing::info!("Successfully scanned and loaded VRChat avatars");
                    Ok(())
                },
                Err(e) => {
                    tracing::error!("Failed to load VRChat avatars: {:?}", e);
                    Err(e)
                }
            }
        } else {
            Err(OscError::Generic("No VRChat avatar watcher is configured".to_string()))
        }
    }
    /// Take the OSC packet receiver to monitor all incoming messages
    pub async fn take_osc_receiver(&self) -> Option<mpsc::UnboundedReceiver<OscPacket>> {
        let mut receiver_guard = self.osc_receiver.lock().await;
        if let Some(ref mut receiver) = *receiver_guard {
            receiver.take_receiver()
        } else {
            None
        }
    }
}

fn is_common_osc_message(addr: &str) -> bool {
    addr.starts_with("/avatar/parameters/") ||
        addr.starts_with("/tracking/")
}