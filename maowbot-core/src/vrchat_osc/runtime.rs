// File: maowbot-core/src/vrchat_osc/runtime.rs
//! The main OSC loop for VRChat. Listens on port 9001 by default, parses OSC data, and
//! dispatches events or toggles. Also handles sending chatbox text, toggling avatar items, etc.

use std::net::SocketAddr;
use tracing::{debug, error, info};
use crate::Error;
use crate::eventbus::{EventBus, BotEvent};
use std::sync::Arc;
use rosc::{OscPacket, OscMessage, OscType};
use tokio::net::UdpSocket;
use tokio::sync::Mutex;
use tokio::time::{interval, Duration};

/// This struct manages OSC I/O for VRChat.
pub struct VrchatOscRuntime {
    event_bus: Arc<EventBus>,
    bind_addr: SocketAddr,
}

impl VrchatOscRuntime {
    /// By default, VRChat sends out OSC on port 9001, listens on port 9000, etc.
    /// We'll bind to 127.0.0.1:9001 to receive VRChat's out-bound messages.
    pub fn new(event_bus: Arc<EventBus>) -> Self {
        let bind = "127.0.0.1:9002".parse().unwrap();
        Self {
            event_bus,
            bind_addr: bind,
        }
    }

    /// The main loop that listens for incoming OSC messages from VRChat.
    /// Then publishes them as BotEvent::ChatMessage or some new variant, etc.
    pub async fn run_main_loop(&mut self) -> Result<(), Error> {
        let socket = UdpSocket::bind(self.bind_addr)
            .await
            .map_err(|e| Error::Platform(format!("Failed to bind VRChat OSC port: {e}")))?;

        info!("VrchatOscRuntime listening on UDP {}", self.bind_addr);

        // We might also run a small heartbeat in parallel
        let mut ticker = interval(Duration::from_secs(5));

        let mut buf = vec![0u8; 1024];
        loop {
            tokio::select! {
                // 1) Incoming OSC
                res = socket.recv_from(&mut buf) => {
                    match res {
                        Ok((size, addr)) => {
                            if let Err(e) = self.handle_incoming_packet(&buf[..size]) {
                                error!("Error handling OSC packet => {:?}", e);
                            }
                        }
                        Err(e) => {
                            error!("Error receiving from VRChat OSC => {:?}", e);
                        }
                    }
                },

                // 2) Periodic tasks
                _ = ticker.tick() => {
                    // Possibly do periodic checks or keepalive logic for OSCQuery
                    debug!("VrchatOscRuntime heartbeat tick...");
                }
            }
        }
    }

    /// Parse an OSC packet and dispatch.
    fn handle_incoming_packet(&self, data: &[u8]) -> Result<(), Error> {
        match rosc::decoder::decode_udp(data) {
            Ok((remain_slice, packet)) => {
                match packet {
                    OscPacket::Message(msg) => {
                        self.handle_osc_message(msg)?;
                    }
                    OscPacket::Bundle(bundle) => {
                        for p in bundle.content {
                            if let OscPacket::Message(m) = p {
                                self.handle_osc_message(m)?;
                            }
                            // ignoring nested bundles for now
                        }
                    }
                }
            }
            Err(e) => {
                return Err(Error::Platform(format!("OSC decode error: {e}")));
            }
        }
        Ok(())
    }

    /// Handle a single OSC message.
    fn handle_osc_message(&self, msg: OscMessage) -> Result<(), Error> {
        let addr = &msg.addr;
        let args = &msg.args;

        // Example: if the message is a chatbox message from VRChat => /chatbox/input ...
        // We'll create a BotEvent::ChatMessage for the event bus.
        if addr == "/chatbox/input" && !args.is_empty() {
            if let Some(OscType::String(text)) = args.get(0) {
                // In VRChat, argument 0 is the text, argument 1 is bool "send"?
                // We'll treat it as a chat message from user "LocalUser" or something
                let user_display = "LocalVRChatUser";
                let channel = "vrchat-osc";  // just a placeholder channel
                let event = BotEvent::ChatMessage {
                    platform: "vrchat-osc".into(),
                    channel: channel.into(),
                    user: user_display.into(),  // no user_id => store string in user field
                    text: text.clone(),
                    timestamp: chrono::Utc::now(),
                };
                let eb = self.event_bus.clone();
                tokio::spawn(async move {
                    eb.publish(event).await;
                });
            }
        }

        // In practice, you'd handle other addresses like /avatar/parameters/SomeToggle, etc.
        // For now, just log them:
        debug!("OSC Message => addr='{}' args={:?}", addr, args);

        Ok(())
    }
}
