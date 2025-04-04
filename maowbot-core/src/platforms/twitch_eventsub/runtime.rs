// twitch_eventsub/runtime.rs

use async_trait::async_trait;
use futures_util::StreamExt;
use tokio::time::{sleep, Duration};
use tokio_tungstenite::tungstenite::protocol::Message;
use tokio_tungstenite::connect_async;
use tracing::{error, info, warn, debug, trace};
use std::sync::Arc;
use reqwest::Client as ReqwestClient;
use serde_json::json;

use crate::Error;
use maowbot_common::models::platform::PlatformCredential;
use maowbot_common::traits::platform_traits::{ConnectionStatus, PlatformAuth, PlatformIntegration};
use crate::eventbus::{EventBus, BotEvent};
use super::events::{
    parse_twitch_notification,
    EventSubNotificationEnvelope,
};

/// TwitchEventSubPlatform holds all relevant state for the websocket session.
pub struct TwitchEventSubPlatform {
    pub credentials: Option<PlatformCredential>,
    pub connection_status: ConnectionStatus,
    pub event_bus: Option<Arc<EventBus>>,
}

impl TwitchEventSubPlatform {
    pub fn new() -> Self {
        Self {
            credentials: None,
            connection_status: ConnectionStatus::Disconnected,
            event_bus: None,
        }
    }

    pub fn set_event_bus(&mut self, event_bus: Arc<EventBus>) {
        self.event_bus = Some(event_bus);
    }

    /// Helper method to check if a WebSocket message is a control frame
    /// (close, ping, or pong).
    fn is_ws_control(msg: &Message) -> bool {
        msg.is_close() || msg.is_ping() || msg.is_pong()
    }

    /// Helper method to determine if a parsed TEXT message is a health check,
    /// i.e. a pong/keepalive/heartbeat message.
    fn is_health_check_message(parsed: &serde_json::Value) -> bool {
        parsed.get("metadata")
            .and_then(|m| m.get("message_type"))
            .and_then(|v| v.as_str())
            .map(|msg_type| {
                msg_type == "session_keepalive" || msg_type == "pong" || msg_type == "heartbeat"
            })
            .unwrap_or(false)
    }

    /// Helper method to log a TEXT message based on its type.
    /// Health check messages are logged only at trace level (if trace is enabled)
    /// while all other messages are logged at debug level.
    fn log_text_message(txt: &str, parsed: &serde_json::Value) {
        if Self::is_health_check_message(parsed) {
            if tracing::enabled!(tracing::Level::TRACE) {
                trace!("[TwitchEventSub] Received TEXT (health check): {}", txt);
            }
        } else {
            debug!("[TwitchEventSub] Received TEXT: {}", txt);
        }
    }

    /// The main loop that attempts to connect to wss://eventsub.wss.twitch.tv/ws
    /// and handle keepalives, notifications, etc.
    pub async fn start_loop(&mut self) -> Result<(), Error> {
        let url = "wss://eventsub.wss.twitch.tv/ws";

        loop {
            // connect
            let connect_result = connect_async(url).await;
            let (mut ws, _resp) = match connect_result {
                Ok(pair) => pair,
                Err(e) => {
                    error!("[TwitchEventSub] WebSocket connect failed: {}", e);
                    self.connection_status = ConnectionStatus::Reconnecting;
                    sleep(Duration::from_secs(15)).await;
                    continue;
                }
            };

            info!("[TwitchEventSub] Connected to {}. Starting read loop...", url);
            self.connection_status = ConnectionStatus::Connected;

            // run_read_loop returns Ok(bool) where the bool indicates "session_reconnect"
            let read_loop_result = self.run_read_loop(&mut ws).await;

            match read_loop_result {
                Ok(need_reconnect) => {
                    if need_reconnect {
                        warn!("[TwitchEventSub] 'session_reconnect' triggered -> reconnecting...");
                        self.connection_status = ConnectionStatus::Reconnecting;
                        // Close the existing connection properly first
                        if let Err(e) = ws.close(None).await {
                            warn!("[TwitchEventSub] Error closing websocket: {}", e);
                        }
                        // Add a small delay before reconnecting to ensure socket is closed
                        sleep(Duration::from_secs(1)).await;
                        continue;
                    } else {
                        info!("[TwitchEventSub] WebSocket read loop ended normally. Exiting loop.");
                        break;
                    }
                }
                Err(e) => {
                    error!("[TwitchEventSub] read loop error => {}", e);
                    self.connection_status = ConnectionStatus::Reconnecting;
                    sleep(Duration::from_secs(15)).await;
                    continue;
                }
            }
        }

        Ok(())
    }

    async fn run_read_loop(
        &mut self,
        ws: &mut tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
    ) -> Result<bool, Error> {
        while let Some(msg_result) = ws.next().await {
            let msg = match msg_result {
                Ok(m) => m,
                Err(e) => {
                    return Err(Error::Platform(format!(
                        "TwitchEventSub WebSocket error: {}",
                        e
                    )));
                }
            };

            // Use helper method to check for WebSocket control frames.
            if Self::is_ws_control(&msg) {
                if tracing::enabled!(tracing::Level::TRACE) {
                    trace!("[TwitchEventSub] WS control frame: {:?}", msg);
                }
                if msg.is_close() {
                    // remote closed
                    return Ok(false);
                }
                continue;
            }

            // handle text messages
            if let Message::Text(txt) = msg {
                let parsed: serde_json::Value = match serde_json::from_str(&txt) {
                    Ok(v) => v,
                    Err(e) => {
                        error!("Could not parse incoming message as JSON => {}", e);
                        continue;
                    }
                };

                // Log TEXT message using helper method that filters health check messages.
                Self::log_text_message(&txt, &parsed);

                let msg_type = parsed
                    .get("metadata")
                    .and_then(|m| m.get("message_type"))
                    .and_then(|v| v.as_str());

                match msg_type {
                    Some("session_welcome") => {
                        info!("[TwitchEventSub] session_welcome => connected OK.");

                        let session_id = parsed
                            .get("payload")
                            .and_then(|p| p.get("session"))
                            .and_then(|s| s.get("id"))
                            .and_then(|id| id.as_str())
                            .unwrap_or("");
                        debug!("[TwitchEventSub] session_id='{}'", session_id);

                        // Attempt to subscribe to events you want:
                        if let Err(e) = self.subscribe_all_events(session_id).await {
                            error!("Error subscribing to events => {:?}", e);
                        }
                    }
                    Some("session_keepalive") => {
                        // Health check message; already logged in log_text_message if trace is enabled.
                        trace!("[TwitchEventSub] session_keepalive => no action needed");
                    }
                    Some("session_reconnect") => {
                        warn!("[TwitchEventSub] session_reconnect => must reconnect soon.");
                        return Ok(true);
                    }
                    Some("notification") => {
                        if let Some(payload) = parsed.get("payload") {
                            let env_res = serde_json::from_value::<EventSubNotificationEnvelope>(payload.clone());
                            let envelope = match env_res {
                                Ok(e) => e,
                                Err(e) => {
                                    error!("Could not parse payload as Envelope => {}", e);
                                    continue;
                                }
                            };
                            let sub_type = &envelope.subscription.sub_type;
                            let event_val = &envelope.event;
                            if let Some(parsed_event) = parse_twitch_notification(sub_type, event_val) {
                                if let Some(bus) = &self.event_bus {
                                    bus.publish(BotEvent::TwitchEventSub(parsed_event)).await;
                                }
                            } else {
                                warn!("[TwitchEventSub] Unknown subscription.type='{}'", sub_type);
                            }
                        }
                    }
                    Some("revocation") => {
                        warn!("[TwitchEventSub] subscription was REVOKED. Possibly missing scope.");
                    }
                    other => {
                        debug!("[TwitchEventSub] Unrecognized message_type={:?}", other);
                    }
                }
            }
        }

        // if we drop out of the while, WS is closed
        Ok(false)
    }

    /// Modify this function to add your new channel points event subscriptions.
    async fn subscribe_all_events(&self, session_id: &str) -> Result<(), Error> {
        let cred = match &self.credentials {
            Some(c) => c,
            None => return Err(Error::Auth("No credential in TwitchEventSubPlatform".into())),
        };
        let access_token = &cred.primary_token;
        let client_id = match cred.additional_data.as_ref()
            .and_then(|v| v.get("client_id"))
            .and_then(|j| j.as_str())
        {
            Some(s) => s.to_string(),
            None => cred.platform_id.clone().unwrap_or_default(), // fallback
        };

        let broadcaster_id = cred.platform_id.clone().unwrap_or_default();
        if broadcaster_id.is_empty() {
            return Err(Error::Auth("No broadcaster user_id in credential.platform_id!".into()));
        }

        let http = ReqwestClient::new();

        // Existing events plus your new channel points events:
        let events_to_subscribe = vec![
            // existing examples:
            ("channel.bits.use", "1",  json!({ "broadcaster_user_id": broadcaster_id })),
            ("channel.update",   "2",     json!({ "broadcaster_user_id": broadcaster_id })),
            ("channel.follow",   "2",     json!({
                "broadcaster_user_id": broadcaster_id,
                "moderator_user_id": broadcaster_id
            })),
            ("channel.ad_break.begin", "1", json!({ "broadcaster_user_id": broadcaster_id })),
            ("channel.chat.notification", "1", json!({
                "broadcaster_user_id": broadcaster_id,
                "user_id": broadcaster_id
            })),
            ("channel.shared_chat.begin",   "1", json!({ "broadcaster_user_id": broadcaster_id })),
            ("channel.shared_chat.update",  "1", json!({ "broadcaster_user_id": broadcaster_id })),
            ("channel.shared_chat.end",     "1", json!({ "broadcaster_user_id": broadcaster_id })),
            ("channel.subscribe", "1", json!({ "broadcaster_user_id": broadcaster_id })),
            ("channel.subscription.end",  "1", json!({ "broadcaster_user_id": broadcaster_id })),
            ("channel.subscription.gift", "1", json!({ "broadcaster_user_id": broadcaster_id })),
            ("channel.subscription.message", "1", json!({ "broadcaster_user_id": broadcaster_id })),
            ("channel.cheer",  "1",  json!({ "broadcaster_user_id": broadcaster_id })),
            ("channel.raid",   "1",  json!({ "to_broadcaster_user_id": broadcaster_id })),
            ("channel.ban",    "1",  json!({ "broadcaster_user_id": broadcaster_id })),
            ("channel.unban",  "1",  json!({ "broadcaster_user_id": broadcaster_id })),
            ("channel.unban_request.create", "1", json!({
                "broadcaster_user_id": broadcaster_id,
                "moderator_user_id": broadcaster_id
            })),
            ("channel.unban_request.resolve", "1", json!({
                "broadcaster_user_id": broadcaster_id,
                "moderator_user_id": broadcaster_id
            })),
            ("channel.hype_train.begin",    "1", json!({ "broadcaster_user_id": broadcaster_id })),
            ("channel.hype_train.progress", "1", json!({ "broadcaster_user_id": broadcaster_id })),
            ("channel.hype_train.end",      "1", json!({ "broadcaster_user_id": broadcaster_id })),
            ("channel.shoutout.create", "1", json!({
                "broadcaster_user_id": broadcaster_id,
                "moderator_user_id": broadcaster_id
            })),
            ("channel.shoutout.receive", "1", json!({
                "broadcaster_user_id": broadcaster_id,
                "moderator_user_id": broadcaster_id
            })),
            ("channel.channel_points_automatic_reward_redemption.add", "beta",
             json!({ "broadcaster_user_id": broadcaster_id })),
            ("channel.channel_points_custom_reward.add", "1",
             json!({ "broadcaster_user_id": broadcaster_id })),
            ("channel.channel_points_custom_reward.update", "1",
             json!({ "broadcaster_user_id": broadcaster_id })),
            ("channel.channel_points_custom_reward.remove", "1",
             json!({ "broadcaster_user_id": broadcaster_id })),
            ("channel.channel_points_custom_reward_redemption.add", "1",
             json!({ "broadcaster_user_id": broadcaster_id })),
            ("channel.channel_points_custom_reward_redemption.update", "1",
             json!({ "broadcaster_user_id": broadcaster_id })),
            ("stream.online", "1",
            json!({"broadcaster_user_id": broadcaster_id })),
            ("stream.offline", "1",
            json!({ "broadcaster_user_id": broadcaster_id })),
        ];

        for (etype, version, condition) in events_to_subscribe {
            let body = json!({
                "type": etype,
                "version": version,
                "condition": condition,
                "transport": {
                    "method": "websocket",
                    "session_id": session_id
                }
            });
            debug!("Subscribing to {} v{} => {:?}", etype, version, body);

            let resp = http
                .post("https://api.twitch.tv/helix/eventsub/subscriptions")
                .header("Client-Id", &client_id)
                .header("Authorization", format!("Bearer {}", access_token))
                .json(&body)
                .send()
                .await
                .map_err(|e| Error::Platform(format!("Error posting subscribe for {etype}: {e}")))?;

            let status = resp.status();
            if !status.is_success() {
                let text = resp.text().await.unwrap_or_default();
                warn!("[TwitchEventSub] Could not subscribe to {} => HTTP {} => {}", etype, status, text);
            } else {
                debug!("[TwitchEventSub] subscribed to {} OK", etype);
            }
        }

        Ok(())
    }
}

#[async_trait]
impl PlatformAuth for TwitchEventSubPlatform {
    async fn authenticate(&mut self) -> Result<(), Error> {
        Ok(())
    }

    async fn refresh_auth(&mut self) -> Result<(), Error> {
        Ok(())
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
impl PlatformIntegration for TwitchEventSubPlatform {
    async fn connect(&mut self) -> Result<(), Error> {
        if matches!(self.connection_status, ConnectionStatus::Connected) {
            return Ok(());
        }
        self.connection_status = ConnectionStatus::Connecting;
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<(), Error> {
        self.connection_status = ConnectionStatus::Disconnected;
        Ok(())
    }

    async fn send_message(&self, _channel: &str, _message: &str) -> Result<(), Error> {
        // EventSub is not a chat interface, so no-op
        Ok(())
    }

    async fn get_connection_status(&self) -> Result<ConnectionStatus, Error> {
        Ok(self.connection_status.clone())
    }
}
