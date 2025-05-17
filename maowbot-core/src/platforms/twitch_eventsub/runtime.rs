// twitch_eventsub/runtime.rs

use async_trait::async_trait;
use futures_util::StreamExt;
use tokio::time::{sleep, Duration};
use tokio_tungstenite::tungstenite::protocol::Message;
use tokio_tungstenite::{connect_async, MaybeTlsStream, WebSocketStream};
use tokio::net::TcpStream;

use tracing::{error, info, warn, debug, trace};
use std::sync::Arc;
use chrono::Utc;
use reqwest::Client as ReqwestClient;
use serde_json::json;

use crate::Error;
use maowbot_common::models::platform::PlatformCredential;
use maowbot_common::traits::auth_traits::PlatformAuthenticator;
use maowbot_common::traits::platform_traits::{ConnectionStatus, PlatformAuth, PlatformIntegration};
use maowbot_common::traits::repository_traits::CredentialsRepository;
use crate::eventbus::{EventBus, BotEvent};
use crate::platforms::twitch_eventsub::TwitchEventSubAuthenticator;
use crate::repositories::postgres::credentials::PostgresCredentialsRepository;
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

    /// Entrypoint — keeps the socket alive and hops when Twitch says so.
    pub async fn start_loop(&mut self) -> Result<(), Error> {
        let mut url = "wss://eventsub.wss.twitch.tv/ws".to_string();   // initial endpoint

        loop {
            let (mut ws, _) = match connect_async(&url).await {
                Ok(pair) => pair,
                Err(e) => {
                    error!("[EventSub] connect error: {e}");
                    self.connection_status = ConnectionStatus::Reconnecting;
                    sleep(Duration::from_secs(15)).await;
                    continue;
                }
            };

            info!("[EventSub] connected → {}", url);
            self.connection_status = ConnectionStatus::Connected;

            match self.run_read_loop(&mut ws).await {
                Ok(Some(new_url)) => {          // Twitch sent session_reconnect
                    warn!("[EventSub] reconnecting → {}", new_url);
                    url = new_url;
                    self.connection_status = ConnectionStatus::Reconnecting;
                    let _ = ws.close(None).await;
                    sleep(Duration::from_millis(500)).await;
                    continue;
                }
                Ok(None) => {                   // graceful close
                    info!("[EventSub] websocket closed.");
                    self.connection_status = ConnectionStatus::Disconnected;
                    break;
                }
                Err(e) => {                     // hard error
                    error!("[EventSub] loop error: {e}");
                    self.connection_status = ConnectionStatus::Reconnecting;
                    sleep(Duration::from_secs(15)).await;
                }
            }
        }
        Ok(())
    }

    /// Reads until the socket closes or a reconnect URL arrives.
    /// `Ok(Some(url))` → caller must reconnect to `url`.
    async fn run_read_loop(
        &mut self,
        ws: &mut WebSocketStream<MaybeTlsStream<TcpStream>>,
    ) -> Result<Option<String>, Error> {
        while let Some(msg_res) = ws.next().await {
            let msg = msg_res.map_err(|e| Error::Platform(format!("ws error: {e}")))?;

            // control frames
            if msg.is_close() { return Ok(None); }
            if msg.is_ping() || msg.is_pong() { continue; }

            // text frames
            let Message::Text(txt) = msg else { continue };
            let parsed: serde_json::Value = serde_json::from_str(&txt)
                .map_err(|e| Error::Platform(format!("bad json: {e}")))?;

            Self::log_text_message(&txt, &parsed);

            match parsed.get("metadata")
                .and_then(|m| m.get("message_type"))
                .and_then(|v| v.as_str()) {
                Some("session_welcome") => {
                    if let Some(id) = parsed.pointer("/payload/session/id").and_then(|v| v.as_str()) {
                        if let Err(e) = self.subscribe_all_events(id).await {
                            error!("subscribe failed: {e:?}");
                        }
                    }
                }
                Some("session_keepalive") => trace!("keepalive"),
                Some("session_reconnect") => {
                    let url = parsed.pointer("/payload/session/reconnect_url")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| Error::Platform("missing reconnect_url".into()))?
                        .to_string();
                    return Ok(Some(url));
                }
                Some("notification") => {
                    if let Some(payload) = parsed.get("payload") {
                        if let Ok(env) = serde_json::from_value::<EventSubNotificationEnvelope>(payload.clone()) {
                            if let Some(evt) = parse_twitch_notification(&env.subscription.sub_type, &env.event) {
                                if let Some(bus) = &self.event_bus {
                                    bus.publish(BotEvent::TwitchEventSub(evt)).await;
                                }
                            }
                        }
                    }
                }
                Some("revocation") => warn!("subscription revoked – check scopes"),
                other => debug!("unhandled message_type={:?}", other),
            }
        }
        Ok(None)        // natural close
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
        
        let mut cred = self
            .credentials
            .clone()
            .ok_or_else(|| Error::Platform("TwitchEventSub: No credential set".into()))?;

        let client_id = cred
            .additional_data
            .as_ref()
            .and_then(|v| v.get("client_id"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .or_else(|| std::env::var("TWITCH_CLIENT_ID").ok())
            .unwrap_or_default();

        let client_secret = std::env::var("TWITCH_CLIENT_SECRET").ok();

        cred = crate::platforms::twitch::requests::token::ensure_valid_token(
            &cred,
            &client_id,
            client_secret.as_deref(),
            600,
        )
            .await?;
        self.credentials = Some(cred);

        // ------------------------------------------------------------------
        // 2) Nothing else to do here – the real socket loop starts later.
        // ------------------------------------------------------------------
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
