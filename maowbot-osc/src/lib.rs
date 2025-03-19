pub mod oscquery;
pub mod vrchat;
pub mod robo;

use std::net::{UdpSocket, SocketAddr};
use std::sync::Arc;
use tokio::sync::{Mutex, mpsc};
use thiserror::Error;
use tokio::task::JoinHandle;

use crate::oscquery::OscQueryServer;
use rosc::{OscPacket, OscType};

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
/// - Spin up an OSC server
/// - Run the OSCQuery server
/// - Parse VRChat's avatar JSON
/// - Maintain a list of toggles
/// - Send toggles & chatbox messages to VRChat
///
/// This is an example integrated approach.
pub struct MaowOscManager {
    pub inner: Arc<Mutex<OscManagerInner>>,
    pub oscquery_server: Arc<Mutex<OscQueryServer>>,
    pub vrchat_watcher: Option<Arc<Mutex<vrchat::avatar_watcher::AvatarWatcher>>>,
    pub osc_receiver: Arc<Mutex<Option<OscReceiver>>>,
}

pub struct OscManagerInner {
    pub listening_port: Option<u16>,
    pub is_running: bool,
    // placeholders for your future expansions
}

#[derive(Debug)]
pub struct OscManagerStatus {
    pub is_running: bool,
    pub listening_port: Option<u16>,
    pub is_oscquery_running: bool,
    pub oscquery_port: Option<u16>,
    pub discovered_peers: Vec<String>,
}

/// Struct to manage receiving OSC messages
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
                                tracing::debug!("Received OSC packet: {} bytes from {}", size, addr);
                                // Parse packet
                                match rosc::decoder::decode_udp(&buf[..size]) {
                                    Ok((_remaining, packet)) => {
                                        // Log received packet for debugging
                                        match &packet {
                                            OscPacket::Message(msg) => {
                                                tracing::debug!("OSC Message: {} with {} args", msg.addr, msg.args.len());
                                            },
                                            OscPacket::Bundle(bundle) => {
                                                tracing::debug!("OSC Bundle with {} messages", bundle.content.len());
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
        };
        // Suppose we want to run the OSCQuery server on port 8080 for HTTP
        let oscquery_server = OscQueryServer::new(8080);

        Self {
            inner: Arc::new(Mutex::new(inner)),
            oscquery_server: Arc::new(Mutex::new(oscquery_server)),
            vrchat_watcher: None,
            osc_receiver: Arc::new(Mutex::new(None)),
        }
    }

    pub async fn get_status(&self) -> Result<OscManagerStatus> {
        // Lock the inner struct here:
        let guard = self.inner.lock().await;
        let oscq_guard = self.oscquery_server.lock().await;

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
        };

        Ok(status)
    }

    /// Starts the OSC server (UDP) and the OSCQuery server (HTTP).
    /// Initiates mDNS advertisement as well.
    ///
    /// **Important**: We now run the mDNS *search* (peer discovery) in a separate
    /// background task, so the server startup doesn't block for its duration.
    pub async fn start_all(&self) -> Result<()> {
        // 1) Start the OSC server on a free UDP port:
        let chosen_port = self.start_server().await?;

        // 2) Start the OSCQuery server:
        {
            let mut oscq = self.oscquery_server.lock().await;
            // Tell OSCQuery to advertise our chosen port for OSC
            oscq.set_osc_port(chosen_port);
            oscq.start().await?;
        }
        tracing::info!("OSCQuery server started on port 8080.");

        // 3) Optionally discover local OSCQuery peers *in background*:
        {
            let oscq_arc = self.oscquery_server.clone();
            tokio::spawn(async move {
                let lock = oscq_arc.lock().await;
                if let Some(discovery) = &lock.discovery {
                    match discovery.discover_peers().await {
                        Ok(found) => {
                            for svc_name in found {
                                tracing::info!("Found local OSCQuery service => {svc_name}");
                            }
                        }
                        Err(e) => {
                            tracing::error!("mDNS discovery error: {:?}", e);
                        }
                    }
                }
            });
        }

        // 4) Start the OSC receiver on the same port we're advertising
        // Note: We're only starting one receiver, and both OscReceiver and AvatarWatcher will share it
        {
            let mut osc_rcv = self.osc_receiver.lock().await;
            if osc_rcv.is_none() {
                match OscReceiver::new(chosen_port) {
                    Ok(receiver) => {
                        *osc_rcv = Some(receiver);
                        tracing::info!("OSC receiver started on port {} (for VRChat)", chosen_port);
                    }
                    Err(e) => {
                        tracing::error!("Failed to start OSC receiver: {:?}", e);
                        tracing::error!("This may happen if port {} is already in use", chosen_port);
                    }
                }
            }
        }

        // 5) Initialize and start VRChat avatar watcher if available
        // The AvatarWatcher will no longer try to bind to 9001 directly
        if let Some(watcher_mutex) = &self.vrchat_watcher {
            let mut watcher = watcher_mutex.lock().await;
            if let Err(e) = watcher.start() {
                tracing::error!("Failed to start VRChat avatar watcher: {:?}", e);
            } else {
                tracing::info!("VRChat avatar watcher started");
            }
        }

        Ok(())
    }

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
            oscq.stop().await?;
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

    /// Attempt to start an OSC server, searching for a free port
    /// (starting at 9002) if needed.
    /// Also start the OSCQuery advertisement if desired.
    pub async fn start_server(&self) -> Result<u16> {
        let mut guard = self.inner.lock().await;
        if guard.is_running {
            return Ok(guard.listening_port.unwrap_or(9002));
        }

        let start_port = 9002;
        let max_port = 9100; // arbitrary upper bound

        let mut port_found = None;
        for port in start_port..max_port {
            // Try binding to all interfaces first
            let addr = format!("0.0.0.0:{port}");
            match UdpSocket::bind(&addr) {
                Ok(_sock) => {
                    port_found = Some(port);
                    break;
                },
                Err(e) => {
                    tracing::debug!("Failed to bind to {}: {}", addr, e);

                    // Try localhost as fallback
                    let localhost_addr = format!("127.0.0.1:{port}");
                    if let Ok(_) = UdpSocket::bind(&localhost_addr) {
                        port_found = Some(port);
                        break;
                    }
                }
            }
        }

        let chosen_port = match port_found {
            Some(p) => p,
            None => return Err(OscError::PortError),
        };

        guard.listening_port = Some(chosen_port);
        guard.is_running = true;
        tracing::info!("OSC server started on port {chosen_port}.");

        // If you want to run your own rosc receiver loop, you could spawn a task here.

        Ok(chosen_port)
    }

    /// Stop the server, if running
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
        let address = "127.0.0.1:9000"; // VRChat typically listens here by default
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