// File: maowbot-core/src/vrchat_osc/oscquery/server.rs
//! A simple skeleton for hosting an OSCQuery server, letting VRChat and other apps
//! discover your custom OSC endpoints automatically. VRChat uses mDNS/Bonjour on the
//! local network for discovery. Windows HTTP security means VRChat can only retrieve
//! your address space from 127.0.0.1 by default, unless you change ACL settings.
//!
//! This code is a placeholder. A real implementation would handle the OSCQuery
//! specification, serve JSON via HTTP, and advertise _osc._udp + _oscjson._tcp via mDNS.

use crate::Error;
use tracing::{info, error};
use tokio::net::TcpListener;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// A placeholder server that might listen for TCP connections to provide
/// OSC address space data in JSON format. Also typically you'd run an mDNS service
/// to advertise your presence on the network.
pub struct OscQueryServer {
    pub port: u16,
    pub is_running: bool,
}

impl OscQueryServer {
    pub fn new(port: u16) -> Self {
        Self {
            port,
            is_running: false,
        }
    }

    pub async fn start(&mut self) -> Result<(), Error> {
        if self.is_running {
            return Ok(());
        }
        let addr = format!("127.0.0.1:{}", self.port);
        let listener = TcpListener::bind(&addr).await
            .map_err(|e| Error::Platform(format!("Failed to bind OSCQuery TCP: {e}")))?;

        self.is_running = true;
        info!("OSCQuery server listening on {}", addr);

        // Minimal example: accept one connection, respond with a JSON stub
        let (mut socket, remote_addr) = listener.accept().await?;
        info!("OSCQuery connection from {}", remote_addr);

        // read minimal request
        let mut buf = [0u8; 1024];
        let n = socket.read(&mut buf).await?;
        let _incoming_req = String::from_utf8_lossy(&buf[..n]);

        // respond with a fake JSON address space
        let fake_json = r#"{
  "OSCQuery.Version": 1,
  "OSCQuery.ExtendedVersion": 1,
  "SERVER_NAME": "MaowBotOSC",
  "OSC_TRANS_PORT": 9000,
  "OSC_RECV_PORT": 9002,
  "CONTENTS": {
    "/avatar/parameters/ToggleExample": {
      "TYPE": "F",
      "ACCESS": "RW"
    }
  }
}"#;

        socket.write_all(fake_json.as_bytes()).await?;
        socket.shutdown().await?;
        Ok(())
    }

    pub fn stop(&mut self) {
        self.is_running = false;
        // In a real server, you’d gracefully close the TCP listener.
    }
}
