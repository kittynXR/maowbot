
pub mod oscquery;
pub mod vrchat;
pub mod robo;

use std::net::UdpSocket;
use std::sync::Arc;
use tokio::sync::Mutex;
use thiserror::Error;

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
        }
    }

    pub async fn get_status(&self) -> Result<OscManagerStatus> {
        // Lock the inner struct here:
        let guard = self.inner.lock().await;

        // We build a simple status object:
        let status = OscManagerStatus {
            is_running: guard.is_running,
            listening_port: guard.listening_port,
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
        let _chosen_port = self.start_server().await?;

        // 2) Start the OSCQuery server:
        {
            let mut oscq = self.oscquery_server.lock().await;
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

        Ok(())
    }

    pub async fn stop_all(&self) -> Result<()> {
        // Stop the UDP OSC
        self.stop_server().await?;

        // Stop the OSCQuery server
        {
            let mut oscq = self.oscquery_server.lock().await;
            oscq.stop().await?;
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
            let addr = format!("127.0.0.1:{port}");
            if let Ok(_sock) = UdpSocket::bind(&addr) {
                // In a real server, we'd keep this socket open and pass it to a rosc listener.
                port_found = Some(port);
                break;
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
        let address = "127.0.0.1:9000"; // VRChat typically listens on this
        let buf = rosc::encoder::encode(&packet)
            .map_err(|e| OscError::IoError(format!("Encode error: {e:?}")))?;

        let sock = UdpSocket::bind(("127.0.0.1", 0))
            .map_err(|e| OscError::IoError(format!("Bind sock error: {e}")))?;
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
}