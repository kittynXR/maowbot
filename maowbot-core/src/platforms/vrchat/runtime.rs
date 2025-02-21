use std::time::Duration;
use async_trait::async_trait;
use futures_util::{StreamExt};
use tokio::select;
use tokio::sync::mpsc::{UnboundedSender, UnboundedReceiver};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio::time::sleep;

use tokio_tungstenite::tungstenite::{
    protocol::Message as WsMessage,
    handshake::client::generate_key,
    http::{Request, Uri, header}
};
use tokio_tungstenite::{connect_async_tls_with_config, Connector};

use crate::Error;
use crate::models::PlatformCredential;
use crate::platforms::{ConnectionStatus, PlatformAuth, PlatformIntegration};

/// This is a simplified VRChat event for demonstration,
/// just capturing (user_id, display_name, text).
#[derive(Debug, Clone)]
pub struct VRChatMessageEvent {
    pub vrchat_display_name: String,
    pub user_id: String,
    pub text: String,
}

/// VRChatPlatform holds the user’s VRChat credential and a background
/// task reading from the pipeline websocket.
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

    /// Connects to wss://pipeline.vrchat.cloud/?authToken=XXXX using
    /// the same handshake approach as your old working code.
    async fn start_websocket_task(&mut self, auth_cookie: &str) -> Result<(), Error> {
        // 1) Extract just the token after "auth="
        let raw_token = match parse_auth_cookie(auth_cookie) {
            Some(t) => t,
            None => {
                return Err(Error::Auth(
                    "Could not find 'auth=' in VRChat cookie".into(),
                ));
            }
        };

        // 2) Build the wss:// URL with ?authToken=...
        let ws_url = format!("wss://pipeline.vrchat.cloud/?authToken={}", raw_token);

        // 3) Construct an explicit handshake request with the same headers
        let uri: Uri = ws_url.parse().map_err(|e| {
            Error::Platform(format!("Invalid VRChat WebSocket URL: {e}"))
        })?;
        let key = generate_key();
        let request = Request::builder()
            .method("GET")
            .uri(uri)
            .header(header::HOST, "pipeline.vrchat.cloud")
            .header(header::ORIGIN, "https://vrchat.com")
            .header(header::USER_AGENT, "MaowBot/1.0 cat@kittyn.cat")
            .header(header::CONNECTION, "Upgrade")
            .header(header::UPGRADE, "websocket")
            .header(header::SEC_WEBSOCKET_VERSION, "13")
            .header(header::SEC_WEBSOCKET_KEY, key)
            .body(())
            .map_err(|e| Error::Platform(format!("Failed to build request: {e}")))?;

        // 4) Connect with TLS config
        let tls_connector = native_tls::TlsConnector::new()
            .map_err(|e| Error::Platform(format!("TlsConnector::new => {e}")))?;
        let connector = Connector::NativeTls(tls_connector);

        let (ws_stream, _response) = connect_async_tls_with_config(
            request,
            None,
            false,
            Some(connector)
        )
            .await
            .map_err(|e| Error::Platform(format!("VRChat WebSocket connect failed: {e}")))?;

        // 5) Split into read/write
        let (mut _write_half, mut read_half) = ws_stream.split();

        // 6) Create the local channel for VRChatMessageEvent
        let (tx_evt, rx_evt) = mpsc::unbounded_channel::<VRChatMessageEvent>();
        self.tx_incoming = Some(tx_evt.clone());
        self.incoming = Some(rx_evt);

        // 7) Spawn a read task that processes messages
        let handle = tokio::spawn(async move {
            tracing::info!("[VRChat] WebSocket read task started.");

            while let Some(incoming) = read_half.next().await {
                match incoming {
                    Ok(WsMessage::Text(txt)) => {
                        if let Some(evt) = parse_vrchat_json_event(&txt) {
                            let _ = tx_evt.send(evt);
                        } else {
                            tracing::debug!("(VRChat) unhandled JSON: {}", txt);
                        }
                    }
                    Ok(WsMessage::Binary(bin)) => {
                        tracing::debug!("(VRChat) got binary message: len={}", bin.len());
                    }
                    Ok(WsMessage::Close(frame)) => {
                        tracing::info!("(VRChat) WebSocket closed by server: {:?}", frame);
                        break;
                    }
                    Ok(_) => {
                        // ping/pong or other
                    }
                    Err(e) => {
                        tracing::warn!("(VRChat) WebSocket error => {}", e);
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

#[async_trait]
impl PlatformAuth for VRChatPlatform {
    async fn authenticate(&mut self) -> Result<(), Error> {
        Ok(())
    }

    async fn refresh_auth(&mut self) -> Result<(), Error> {
        Err(Error::Auth(
            "VRChat does not support refresh flow.".into(),
        ))
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

        let cookie_str = cred.primary_token.clone(); // something like "auth=XYZ; path=/; etc..."
        self.start_websocket_task(&cookie_str).await?;

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
        Ok(())
    }

    async fn get_connection_status(&self) -> Result<crate::platforms::ConnectionStatus, Error> {
        Ok(self.connection_status.clone())
    }
}

/// Minimal JSON parse that tries to see if it's "user-update" or "user-location"
fn parse_vrchat_json_event(raw_json: &str) -> Option<VRChatMessageEvent> {
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
        // ... more types ...
        _ => None,
    }
}

/// Splits out the "auth=" portion from a cookie string like
/// `"auth=ABCDEF; Path=/; HttpOnly; ..."`
fn parse_auth_cookie(cookie_str: &str) -> Option<String> {
    cookie_str
        .split(';')
        .find_map(|piece| {
            let trimmed = piece.trim();
            if trimmed.starts_with("auth=") {
                Some(trimmed.trim_start_matches("auth=").to_string())
            } else {
                None
            }
        })
}