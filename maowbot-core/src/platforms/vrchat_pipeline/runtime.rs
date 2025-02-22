// File: maowbot-core/src/platforms/vrchat_pipeline/runtime.rs
//
// This file was moved from the old src/platforms/vrchat/runtime.rs,
// which handles the WebSocket/pipeline logic.

use std::time::Duration;
use async_trait::async_trait;
use futures_util::StreamExt;
use tokio::{select, time::sleep};
use tokio::sync::mpsc::{UnboundedSender, UnboundedReceiver};
use tokio::sync::{mpsc, Mutex};
use tokio::task::JoinHandle;
use tokio_tungstenite::tungstenite::{
    protocol::Message as WsMessage,
    handshake::client::generate_key,
    http::{Request, Uri, header}
};
use tokio_tungstenite::{connect_async_tls_with_config, Connector};

use crate::Error;
use crate::models::PlatformCredential;
use crate::platforms::{ConnectionStatus, PlatformAuth, PlatformIntegration};
use crate::platforms::vrchat::auth::parse_auth_cookie_from_headers;

/// This is a simplified VRChat event for demonstration,
/// capturing (user_id, display_name, text).
#[derive(Debug, Clone)]
pub struct VRChatMessageEvent {
    pub vrchat_display_name: String,
    pub user_id: String,
    pub text: String,
}

/// VRChatPlatform is the pipeline-based platform object: handles the
/// background WebSocket reading for VRChat events.
pub struct VRChatPlatform {
    pub credentials: Option<PlatformCredential>,
    pub connection_status: ConnectionStatus,

    incoming: Option<UnboundedReceiver<VRChatMessageEvent>>,
    tx_incoming: Option<UnboundedSender<VRChatMessageEvent>>,

    /// The task reading from the websocket
    read_task: Option<JoinHandle<()>>,

    /// For clean shutdown, store a one-shot sender
    write_shutdown_handle: Option<tokio::sync::oneshot::Sender<()>>,
}

impl VRChatPlatform {
    pub fn new() -> Self {
        Self {
            credentials: None,
            connection_status: ConnectionStatus::Disconnected,
            incoming: None,
            tx_incoming: None,
            read_task: None,
            write_shutdown_handle: None,
        }
    }

    pub async fn next_message_event(&mut self) -> Option<VRChatMessageEvent> {
        if let Some(ref mut rx) = self.incoming {
            rx.recv().await
        } else {
            None
        }
    }

    /// Starts the background read loop from wss://pipeline.vrchat.cloud
    async fn start_websocket_task(&mut self, auth_cookie: &str) -> Result<(), Error> {
        // 1) Extract token after "auth="
        let raw_token = match extract_auth_token(auth_cookie) {
            Some(t) => t,
            None => {
                return Err(Error::Auth(
                    "Could not find 'auth=' in VRChat cookie".into(),
                ));
            }
        };

        // 2) Build the wss:// URL with ?authToken=...
        let ws_url = format!("wss://pipeline.vrchat.cloud/?authToken={}", raw_token);

        // 3) Build handshake request
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

        let (mut write_half, mut read_half) = ws_stream.split();

        // 5) Create local channel for VRChatMessageEvent
        let (tx_evt, rx_evt) = mpsc::unbounded_channel::<VRChatMessageEvent>();
        self.tx_incoming = Some(tx_evt.clone());
        self.incoming = Some(rx_evt);

        // 6) Create shutdown channel for writing half
        let (shutdown_tx, mut shutdown_rx) = tokio::sync::oneshot::channel::<()>();
        self.write_shutdown_handle = Some(shutdown_tx);

        // 7) Spawn the read loop
        let handle = tokio::spawn(async move {
            tracing::info!("[VRChat] WebSocket read task started.");

            loop {
                select! {
                    maybe_msg = read_half.next() => {
                        match maybe_msg {
                            Some(Ok(WsMessage::Text(txt))) => {
                                if let Some(evt) = parse_vrchat_json_event(&txt) {
                                    let _ = tx_evt.send(evt);
                                } else {
                                    tracing::debug!("(VRChat) unhandled JSON: {}", txt);
                                }
                            }
                            Some(Ok(WsMessage::Binary(bin))) => {
                                tracing::debug!("(VRChat) got binary message: len={}", bin.len());
                            }
                            Some(Ok(WsMessage::Close(frame))) => {
                                tracing::info!("(VRChat) WebSocket closed by server: {:?}", frame);
                                break;
                            }
                            Some(Ok(_other)) => {
                                // ping/pong or other messages
                            }
                            Some(Err(e)) => {
                                tracing::warn!("(VRChat) WebSocket error => {}", e);
                                break;
                            }
                            None => {
                                // Stream ended
                                tracing::info!("(VRChat) WebSocket stream ended.");
                                break;
                            }
                        }
                    }
                    _ = &mut shutdown_rx => {
                        tracing::info!("(VRChat) Received shutdown signal. Closing read loop.");
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
        // No-op here; any real login is done outside
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

        let cookie_str = cred.primary_token.clone(); // "auth=AbCdEf..."
        self.start_websocket_task(&cookie_str).await?;

        self.connection_status = ConnectionStatus::Connected;
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<(), Error> {
        self.connection_status = ConnectionStatus::Disconnected;
        // Send a shutdown signal
        if let Some(tx) = self.write_shutdown_handle.take() {
            let _ = tx.send(());
        }
        // Then abort the read task if still running
        if let Some(handle) = self.read_task.take() {
            handle.abort();
        }
        Ok(())
    }

    async fn send_message(&self, _channel: &str, _message: &str) -> Result<(), Error> {
        // VRChat pipeline is primarily one-way. No direct text chat sending here.
        Ok(())
    }

    async fn get_connection_status(&self) -> Result<ConnectionStatus, Error> {
        Ok(self.connection_status.clone())
    }
}

/// Minimal JSON parse to see if it's "user-update" or "user-location"
fn parse_vrchat_json_event(raw_json: &str) -> Option<VRChatMessageEvent> {
    let json_val: serde_json::Value = serde_json::from_str(raw_json).ok()?;
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
        _ => None,
    }
}

/// Splits out "auth=..." from a cookie string like
/// `"auth=ABC123; Path=/; HttpOnly; ..."`.
fn extract_auth_token(cookie_str: &str) -> Option<String> {
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