//! maowbot-osc/src/vrchat/chatbox.rs
//!
//! Helper functions or structs for operating VRChat's chatbox via OSC.
//!   /chatbox/input <text> <bool> <bool>
//!   /chatbox/typing <bool>
//!
//! The first bool: true => send immediately, false => open VR keyboard with pre-filled text
//! The second bool: whether to play the notification sound (defaults true if omitted).

use crate::{Result, OscError, MaowOscManager};
use rosc::{OscPacket, OscMessage, OscType};
use std::net::UdpSocket;

/// Holds data for chatbox input.
pub struct ChatboxMessage {
    pub text: String,
    pub send_immediately: bool,
    pub play_notification_sound: bool,
}

impl ChatboxMessage {
    pub fn new(text: &str, send_immediately: bool) -> Self {
        Self {
            text: text.to_string(),
            send_immediately,
            play_notification_sound: true,
        }
    }
}

/// Additional chatbox-related methods to be invoked. You can implement them
/// as free functions or as trait impl. Here, for convenience, we show them
/// as top-level standalone functions.

/// Send a message to VRChat chatbox. Address => /chatbox/input s b n
/// - s: string text
/// - b: bool (true => send immediately)
/// - n: bool (true => play sound)
pub fn send_chatbox_message(osc_manager: &MaowOscManager, msg: &ChatboxMessage) -> Result<()> {
    // Build the packet
    let osc_msg = OscMessage {
        addr: "/chatbox/input".to_string(),
        args: vec![
            OscType::String(msg.text.clone()),
            OscType::Bool(msg.send_immediately),
            OscType::Bool(msg.play_notification_sound),
        ],
    };
    let packet = OscPacket::Message(osc_msg);

    // Reuse the manager's internal logic to send
    send_packet_to_vrchat(packet)
}

/// Toggle the chatbox "typing" indicator on or off. Address => /chatbox/typing b
pub fn set_chatbox_typing(osc_manager: &MaowOscManager, typing_on: bool) -> Result<()> {
    let osc_msg = OscMessage {
        addr: "/chatbox/typing".to_string(),
        args: vec![OscType::Bool(typing_on)],
    };
    let packet = OscPacket::Message(osc_msg);

    send_packet_to_vrchat(packet)
}

/// Minimal helper that sends the given packet to VRChat's default port (9000).
fn send_packet_to_vrchat(packet: OscPacket) -> Result<()> {
    let address = "127.0.0.1:9000"; // VRChat listens here by default
    let buf = rosc::encoder::encode(&packet)
        .map_err(|e| OscError::IoError(format!("Encode error: {e:?}")))?;

    let sock = UdpSocket::bind(("127.0.0.1", 0))
        .map_err(|e| OscError::IoError(format!("Bind sock error: {e}")))?;
    sock.send_to(&buf, address)
        .map_err(|e| OscError::IoError(format!("Send error: {e}")))?;
    Ok(())
}
