// File: maowbot-core/src/vrchat_osc/toggles/mod.rs
//! Supports controlling avatar toggles (Boolean or Int parameters) via OSC.
//! This might be triggered by chat commands, UI interactions, or plugin calls.

use crate::Error;
use tracing::info;
use rosc::{OscPacket, OscMessage, OscType};
use std::net::SocketAddr;
use tokio::net::UdpSocket;

/// A simple function to send an OSC bool toggle to VRChat on port 9000 (the default input).
pub async fn send_toggle(parameter_name: &str, value: bool) -> Result<(), Error> {
    // e.g. "/avatar/parameters/HatToggle" with bool
    let addr = format!("/avatar/parameters/{}", parameter_name);

    let msg = OscMessage {
        addr,
        args: vec![OscType::Bool(value)],
    };
    let packet = OscPacket::Message(msg);
    let encoded = rosc::encoder::encode(&packet)
        .map_err(|e| Error::Platform(format!("OSC encode error: {e}")))?;

    let target_addr: SocketAddr = "127.0.0.1:9000".parse().unwrap();
    let sock = UdpSocket::bind("127.0.0.1:0").await?; // ephemeral local port
    sock.send_to(&encoded, target_addr).await?;
    info!("Sent toggle parameter='{}' value={}", parameter_name, value);
    Ok(())
}
