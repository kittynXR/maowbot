use std::time::Duration;
use async_trait::async_trait;
use futures_util::{StreamExt};
use tokio::select;
use tokio::sync::mpsc::{UnboundedSender, UnboundedReceiver};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio::time::sleep;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};

use crate::Error;
use crate::models::PlatformCredential;
use crate::platforms::{ConnectionStatus, PlatformAuth, PlatformIntegration};

/// This is a simplified VRChat event for demonstration,
/// just capturing (user_id, display_name, text).
/// In real code, you might parse out the JSON event types more precisely.
#[derive(Debug, Clone)]
pub struct VRChatMessageEvent {
    pub vrchat_display_name: String,
    pub user_id: String,
    pub text: String,
}

/// VRChatPlatform holds the user’s VRChat credential and a background
/// task reading from the pipeline websocket.
/// We’ll publish parsed events into `incoming` so
/// the TUI or an event-bus-based consumer can handle them.
pub struct VRChatPlatform {
    pub(crate) credentials: Option<PlatformCredential>,
    pub(crate) connection_status: ConnectionStatus,

    incoming: Option<UnboundedReceiver<VRChatMessageEvent>>,
    tx_incoming: Option<UnboundedSender<VRChatMessageEvent>>,

    /// The task reading from the websocket
    read_task: Option<JoinHandle<()>>,
}

impl VRChatPlatform {
    pub fn new() -> Self {
        Self {
            credentials: None,
            connection_status: ConnectionStatus::Disconnected,
            incoming: None,
            tx_incoming: None,
            read_task: None,
        }
    }

    pub async fn next_message_event(&mut self) -> Option<VRChatMessageEvent> {
        if let Some(ref mut rx) = self.incoming {
            rx.recv().await
        } else {
            None
        }
    }

    /// The wss endpoint for VRChat user events is typically:
    /// `wss://pipeline.vrchat.cloud/?authToken=<tokenOrCookie>`,
    /// though you might see variations in VRChat docs.
    /// We’ll attempt to parse the “auth=xxx” from `primary_token`
    /// and attach it as the `authToken` parameter.
    async fn start_websocket_task(
        &mut self,
        primary_token: String,
    ) -> Result<(), Error> {
        // Example: primary_token = "auth=XXXXXXXXXXXXXXXXXXXXX"
        // We only need the raw portion after `auth=`
        let raw_cookie_val = primary_token.strip_prefix("auth=").unwrap_or("");
        if raw_cookie_val.is_empty() {
            return Err(Error::Auth("Missing VRChat 'auth=' token".into()));
        }
        // Build the pipeline WebSocket URL
        let ws_url = format!("wss://pipeline.vrchat.cloud/?authToken={}", raw_cookie_val);

        let (tx_evt, rx_evt) = mpsc::unbounded_channel::<VRChatMessageEvent>();
        self.tx_incoming = Some(tx_evt);
        self.incoming = Some(rx_evt);

        // Connect the websocket
        let (ws_stream, _response) = connect_async(&ws_url).await.map_err(|e| {
            Error::Platform(format!("VRChat WebSocket connect failed: {e}"))
        })?;

        let (mut write_half, mut read_half) = ws_stream.split();

        // We don’t typically *send* messages to VRChat pipeline
        // (it’s one-way), but you could keep `write_half` around if needed.

        // Spawn a read task
        let tx_for_task = self.tx_incoming.clone().unwrap();
        let handle = tokio::spawn(async move {
            tracing::info!("[VRChat] WebSocket read task started.");

            loop {
                select! {
                    msg_opt = read_half.next() => {
                        match msg_opt {
                            Some(Ok(msg)) => {
                                if let Message::Text(txt) = msg {
                                    let parsed_opt = parse_vrchat_json_event(&txt);
                                    if let Some(evt) = parsed_opt {
                                        let _ = tx_for_task.send(evt);
                                    } else {
                                        tracing::debug!("(VRChat) unhandled JSON: {}", txt);
                                    }
                                }
                                else if let Message::Binary(bin) = msg {
                                    tracing::debug!("(VRChat) got binary message: len={}", bin.len());
                                }
                                else if let Message::Close(_frame) = msg {
                                    tracing::info!("(VRChat) WebSocket closed by server.");
                                    break;
                                }
                                else {
                                    // ping/pong or others
                                    // do nothing
                                }
                            },
                            Some(Err(e)) => {
                                tracing::warn!("(VRChat) websocket error => {}", e);
                                break;
                            },
                            None => {
                                // EOF
                                break;
                            }
                        }
                    }
                    // We can also watch for shutdown or other signals here
                    else => {
                        tracing::warn!("(VRChat) read task => no more events?");
                        break;
                    }
                }
            }

            tracing::info!("[VRChat] WebSocket read task ended.");
        });
        self.read_task = Some(handle);

        Ok(())
    }
}

/// A minimal parse that tries to see if it's a user-update or user-location
/// or similar. Then we produce a VRChatMessageEvent for the TUI.
fn parse_vrchat_json_event(raw_json: &str) -> Option<VRChatMessageEvent> {
    // For example, if we see a "type": "user-update", we can decode displayName, etc.
    let json_val: serde_json::Value = match serde_json::from_str(raw_json) {
        Ok(v) => v,
        Err(_) => return None,
    };
    let event_type = json_val["type"].as_str().unwrap_or("");
    let content = &json_val["content"];

    match event_type {
        "user-update" => {
            let user_obj = &content["user"];
            let display_name = user_obj["displayName"].as_str().unwrap_or("Unknown").to_string();
            let user_id = user_obj["id"].as_str().unwrap_or("???").to_string();

            let text = format!("VRChat user-update => status={}", user_obj["status"].as_str().unwrap_or(""));
            Some(VRChatMessageEvent {
                vrchat_display_name: display_name,
                user_id,
                text,
            })
        }
        "user-location" => {
            let user_obj = &content["user"];
            let display_name = user_obj["displayName"].as_str().unwrap_or("Unknown").to_string();
            let user_id = user_obj["id"].as_str().unwrap_or("???").to_string();

            let location = content["location"].as_str().unwrap_or("???");
            let text = format!("Changed location => {}", location);

            Some(VRChatMessageEvent {
                vrchat_display_name: display_name,
                user_id,
                text,
            })
        }
        // ... handle other event types as needed ...
        _ => None,
    }
}

#[async_trait]
impl PlatformAuth for VRChatPlatform {
    async fn authenticate(&mut self) -> Result<(), Error> {
        // typically no-op; see VRChatAuthenticator for the real logic
        Ok(())
    }

    async fn refresh_auth(&mut self) -> Result<(), Error> {
        Err(Error::Auth("VRChat does not support refresh flow.".into()))
    }

    async fn revoke_auth(&mut self) -> Result<(), Error> {
        self.credentials = None;
        Ok(())
    }

    async fn is_authenticated(&self) -> Result<bool, Error> {
        Ok(self.credentials.is_some())
    }
}

#[async_trait]
impl PlatformIntegration for VRChatPlatform {
    async fn connect(&mut self) -> Result<(), Error> {
        if self.connection_status == ConnectionStatus::Connected {
            return Ok(()); // already connected
        }
        let cred = self
            .credentials
            .as_ref()
            .ok_or_else(|| Error::Platform("VRChat: No credentials set".into()))?;
        let token = cred.primary_token.clone(); // "auth=XXXX"

        // Start the WS read loop
        self.start_websocket_task(token).await?;

        self.connection_status = ConnectionStatus::Connected;
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<(), Error> {
        self.connection_status = ConnectionStatus::Disconnected;
        if let Some(handle) = self.read_task.take() {
            handle.abort();
        }
        Ok(())
    }

    async fn send_message(&self, _channel: &str, _message: &str) -> Result<(), Error> {
        // VRChat pipeline is primarily one-way, no direct text chat sending.
        // If you needed to do invites or instance transitions, you'd call the
        // VRChat REST endpoints. This is a stub.
        Ok(())
    }

    async fn get_connection_status(&self) -> Result<crate::platforms::ConnectionStatus, Error> {
        Ok(self.connection_status.clone())
    }
}
