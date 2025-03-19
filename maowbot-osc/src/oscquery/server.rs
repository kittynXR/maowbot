use crate::{Result};
use crate::oscquery::discovery::OscQueryDiscovery;
use warp::Filter;
use std::net::{SocketAddr, Ipv4Addr};
use tracing::{info, error};
use std::collections::HashMap;
use serde_json::json;

/// An OSCQuery HTTP server that can optionally advertise itself via mDNS.
/// By default, it listens on a chosen `http_port`. Once started, it serves
/// a minimal JSON response describing some OSC endpoints.
///
/// If you wish to discover or advertise other local services, see `OscQueryDiscovery`.
pub struct OscQueryServer {
    pub is_running: bool,
    pub http_port: u16,
    pub osc_port: u16, // New field to store the OSC port to advertise
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
            osc_port: 9001, // Default to 9001 for OSC (VRChat's default output port)
            stop_tx: None,
            discovery: None,
        }
    }

    /// Set the OSC port to advertise via OSCQuery
    pub fn set_osc_port(&mut self, port: u16) {
        self.osc_port = port;
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

        // Create properties including the OSC port to tell VRChat where to send messages
        let osc_port = self.osc_port; // Store for capture in routes

        // Create a minimal route returning an OSCQuery JSON structure
        // that correctly advertises our actual listening port
        let route = warp::path::end().map(move || {
            let response = json!({
                "DESCRIPTION": "MaowBot OSC Server",
                "ACCESS": {
                    "read": true,
                    "write": true,
                    "value": true
                },
                "OSC_PORT": osc_port,
                "EXTENSIONS": {
                    "ACCESS": true,
                    "VALUE": true,
                    "RANGE": true,
                    "DESCRIPTION": true,
                    "TAGS": true,
                    "EXTENDED_TYPE": true,
                    "UNIT": true,
                    "CRITICAL": true,
                    "CLIPMODE": true
                },
                "CONTENTS": {
                    "avatar": {
                        "FULL_PATH": "/avatar",
                        "ACCESS": {
                            "read": true,
                            "write": true,
                            "value": true
                        },
                        "CONTENTS": {
                            "parameters": {
                                "FULL_PATH": "/avatar/parameters",
                                "ACCESS": {
                                    "read": true,
                                    "write": true,
                                    "value": true
                                },
                                "CONTENTS": {
                                    "Example": {
                                        "FULL_PATH": "/avatar/parameters/Example",
                                        "TYPE": "f",
                                        "ACCESS": {
                                            "read": true,
                                            "write": true,
                                            "value": true
                                        },
                                        "VALUE": 0.0
                                    }
                                }
                            },
                            "change": {
                                "FULL_PATH": "/avatar/change",
                                "TYPE": "s",
                                "ACCESS": {
                                    "read": true,
                                    "write": true,
                                    "value": true
                                }
                            }
                        }
                    },
                    "chatbox": {
                        "FULL_PATH": "/chatbox",
                        "ACCESS": {
                            "read": true,
                            "write": true,
                            "value": true
                        },
                        "CONTENTS": {
                            "input": {
                                "FULL_PATH": "/chatbox/input",
                                "TYPE": "sbb",
                                "ACCESS": {
                                    "read": true,
                                    "write": true,
                                    "value": true
                                }
                            },
                            "typing": {
                                "FULL_PATH": "/chatbox/typing",
                                "TYPE": "b",
                                "ACCESS": {
                                    "read": true,
                                    "write": true,
                                    "value": true
                                }
                            }
                        }
                    }
                }
            });
            warp::reply::json(&response)
        });

        // Add a path handler for HOST_INFO
        let host_info_route = warp::path("host_info").map(move || {
            let response = json!({
                "NAME": "MaowBot OSC Server",
                "OSC_IP": "127.0.0.1",
                "OSC_PORT": osc_port,
                "OSC_TRANSPORT": "UDP",
                "EXTENSIONS": {
                    "ACCESS": true,
                    "VALUE": true,
                    "RANGE": true,
                    "DESCRIPTION": true,
                    "TAGS": true,
                    "EXTENDED_TYPE": true,
                    "UNIT": true,
                    "CRITICAL": true,
                    "CLIPMODE": true
                }
            });
            warp::reply::json(&response)
        });

        let (stop_tx, stop_rx) = tokio::sync::oneshot::channel();
        self.stop_tx = Some(stop_tx);

        // Use 0.0.0.0 so it binds all local IPv4 addresses
        let ip = Ipv4Addr::UNSPECIFIED;
        let addr = SocketAddr::new(ip.into(), self.http_port);

        // Combine routes and start the warp server with graceful shutdown
        let routes = route.or(host_info_route);
        let (server_addr, server_future) = warp::serve(routes)
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
                    // Create properties to advertise our OSC port
                    let mut properties = HashMap::new();
                    properties.insert("OSC_PORT".to_string(), self.osc_port.to_string());
                    properties.insert("OSC_TRANSPORT".to_string(), "UDP".to_string());
                    properties.insert("OSC_IP".to_string(), "127.0.0.1".to_string());

                    if let Err(e) = d.advertise_with_properties("MaowBotOSCQuery", self.http_port, properties).await {
                        error!("Failed to advertise mDNS for OSCQuery: {:?}", e);
                    } else {
                        info!("mDNS advertisement started for OSCQuery on HTTP port {} / OSC port {}",
                             self.http_port, self.osc_port);
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