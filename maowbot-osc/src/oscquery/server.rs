use crate::{Result};
use crate::oscquery::discovery::OscQueryDiscovery;
use warp::Filter;
use std::net::{SocketAddr, Ipv4Addr};
use tracing::{info, error};

/// An OSCQuery HTTP server that can optionally advertise itself via mDNS.
/// By default, it listens on a chosen `http_port`. Once started, it serves
/// a minimal JSON response describing some OSC endpoints.
///
/// If you wish to discover or advertise other local services, see `OscQueryDiscovery`.
pub struct OscQueryServer {
    pub is_running: bool,
    pub http_port: u16,
    stop_tx: Option<tokio::sync::oneshot::Sender<()>>,
    pub discovery: Option<OscQueryDiscovery>,
}

impl OscQueryServer {
    /// Create a new server for the given HTTP port.
    /// Note: This doesn't automatically start the server or do any mDNS advertisement yet.
    pub fn new(port: u16) -> Self {
        Self {
            is_running: false,
            http_port: port,
            stop_tx: None,
            discovery: None,
        }
    }

    pub fn is_running(&self) -> bool {
        self.is_running
    }
    /// Start the OSCQuery HTTP server using `warp`. Also attempts to
    /// advertise via mDNS if possible.
    pub async fn start(&mut self) -> Result<()> {
        if self.is_running {
            return Ok(());
        }
        self.is_running = true;

        // Create a minimal route returning some OSCQuery JSON structure
        let route = warp::path::end().map(|| {
            warp::reply::json(&serde_json::json!({
                "hello": "from MaowBot",
                "addresses": ["/avatar/parameters/Example"]
            }))
        });

        let (stop_tx, stop_rx) = tokio::sync::oneshot::channel();
        self.stop_tx = Some(stop_tx);

        // Use 0.0.0.0 so it binds all local IPv4 addresses
        let ip = Ipv4Addr::UNSPECIFIED;
        let addr = SocketAddr::new(ip.into(), self.http_port);

        // Start the warp server with graceful shutdown
        let (server_addr, server_future) = warp::serve(route)
            .bind_with_graceful_shutdown(addr, async move {
                let _ = stop_rx.await;
            });

        info!("Starting OSCQuery HTTP server on http://{}", server_addr);

        // Spawn the warp server in background
        tokio::spawn(async move {
            server_future.await;
            info!("OSCQuery HTTP server shut down.");
        });

        // Attempt to create and start advertisement:
        match OscQueryDiscovery::new() {
            Ok(discovery) => {
                // We'll store it in our struct, so we can stop it later
                self.discovery = Some(discovery);
                if let Some(d) = &self.discovery {
                    if let Err(e) = d.advertise("MaowBotOSCQuery", self.http_port).await {
                        error!("Failed to advertise mDNS for OSCQuery: {:?}", e);
                    } else {
                        info!("mDNS advertisement started for OSCQuery on port {}", self.http_port);
                    }
                }
            }
            Err(e) => {
                error!("Failed to initialize mDNS: {:?}", e);
            }
        }

        Ok(())
    }

    /// Stop the server gracefully, and stop any mDNS advertising too.
    pub async fn stop(&mut self) -> Result<()> {
        if !self.is_running {
            return Ok(());
        }
        self.is_running = false;

        // Stop the warp server
        if let Some(tx) = self.stop_tx.take() {
            let _ = tx.send(());
        }

        // Stop any mDNS advertisement
        if let Some(disc) = &self.discovery {
            if let Err(e) = disc.stop() {
                error!("Error stopping mDNS advertisement: {:?}", e);
            }
        }
        self.discovery = None;

        Ok(())
    }
}
