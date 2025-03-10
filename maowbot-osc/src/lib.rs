//! maowbot-osc/src/lib.rs
//!
//! The main library file for the `maowbot-osc` crate.
//! Re-exports major submodules.

pub mod oscquery;
pub mod vrchat;
pub mod robo;

use std::sync::Arc;
use tokio::sync::Mutex;
use thiserror::Error;

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
/// - spin up an OSC server
/// - run the OSCQuery discovery
/// - parse VRChat's avatar JSON
/// - maintain a list of toggles
///
/// This is a placeholder skeleton for your integrated approach.
pub struct MaowOscManager {
    inner: Arc<Mutex<OscManagerInner>>,
}

struct OscManagerInner {
    pub listening_port: Option<u16>,
    pub is_running: bool,
    // placeholders for your future expansions
}

impl MaowOscManager {
    pub fn new() -> Self {
        let inner = OscManagerInner {
            listening_port: None,
            is_running: false,
        };
        Self {
            inner: Arc::new(Mutex::new(inner))
        }
    }

    /// Attempt to start an OSC server, searching for a free port
    /// (starting at 9002) if needed.
    /// Also start the OSCQuery advertisement if desired.
    pub async fn start_server(&self) -> Result<u16> {
        // minimal placeholder
        let mut guard = self.inner.lock().await;
        if guard.is_running {
            return Ok(guard.listening_port.unwrap_or(9002));
        }

        let start_port = 9002;
        let max_port = 9100; // arbitrary upper bound

        // Example: try multiple ports until success
        let mut port_found = None;
        for port in start_port..max_port {
            // You might attempt to bind UDP here. If successful, store port and break.
            // rosc uses "SocketAddr" etc.
            let addr = format!("127.0.0.1:{port}");
            if let Ok(_sock) = std::net::UdpSocket::bind(&addr) {
                // We won't actually keep _sock in this skeleton.
                port_found = Some(port);
                break;
            }
        }

        let chosen_port = match port_found {
            Some(p) => p,
            None => {
                return Err(OscError::PortError);
            }
        };

        // In real usage, you'd pass that open socket to a rosc server task
        // that listens and processes messages.
        guard.listening_port = Some(chosen_port);
        guard.is_running = true;

        tracing::info!("OSC server started on port {chosen_port}.");

        // Start OSCQuery advertisement here, if desired
        // (mdns or other approach).

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

    /// A placeholder for sending a single OSC message to VRChat
    /// (e.g., toggling an avatar parameter).
    pub async fn send_osc_toggle(&self, param_name: &str, value: f32) -> Result<()> {
        let guard = self.inner.lock().await;
        let port = guard.listening_port.unwrap_or(9002);
        let address = format!("127.0.0.1:9000"); // VRChat typically listens on 9000

        // Build rosc::OscPacket
        let osc_msg = rosc::OscMessage {
            addr: format!("/avatar/parameters/{}", param_name),
            args: vec![rosc::OscType::Float(value)],
        };
        let packet = rosc::OscPacket::Message(osc_msg);

        let buf = rosc::encoder::encode(&packet)
            .map_err(|e| OscError::IoError(format!("Encode error: {e:?}")))?;

        // Send the packet
        let sock = std::net::UdpSocket::bind(("127.0.0.1", 0))
            .map_err(|e| OscError::IoError(format!("Bind sock error: {e}")))?;
        sock.send_to(&buf, &address)
            .map_err(|e| OscError::IoError(format!("Send error: {e}")))?;

        tracing::debug!("Sent OSC toggle => param={param_name}, value={value}, to VRChat:9000");
        Ok(())
    }
}
