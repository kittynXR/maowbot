// src/plugins/plugin_connection.rs
use std::any::Any;
use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::mpsc::UnboundedSender;

use crate::Error;
use maowbot_proto::plugs::PluginStreamResponse;
use maowbot_proto::plugs::plugin_stream_response::Payload as RespPayload;

use tracing::info;
use crate::plugins::types::PluginType;
use crate::plugins::bot_api::{BotApi};

/// PluginConnectionInfo: in-memory info about a connected plugin.
#[derive(Clone)]
pub struct PluginConnectionInfo {
    pub name: String,
    pub capabilities: Vec<maowbot_proto::plugs::PluginCapability>,
    pub is_enabled: bool,
}

/// Trait for any plugin connection (in-process or gRPC).
#[async_trait]
pub trait PluginConnection: Send + Sync {
    /// Return a copy of this connection’s info.
    async fn info(&self) -> PluginConnectionInfo;

    /// Update the connection’s stored capabilities.
    async fn set_capabilities(&self, capabilities: Vec<maowbot_proto::plugs::PluginCapability>);

    /// Change the plugin’s displayed name.
    async fn set_name(&self, new_name: String);

    /// Send a `PluginStreamResponse` message to the plugin.
    async fn send(&self, response: PluginStreamResponse) -> Result<(), Error>;

    /// Called when removing or shutting down the plugin.
    async fn stop(&self) -> Result<(), Error>;

    /// Provide an API object if the plugin needs to call back into the bot.
    fn set_bot_api(&self, _api: Arc<dyn BotApi>) {}

    /// Enable or disable the plugin (the plugin may ignore sends when disabled).
    async fn set_enabled(&self, enable: bool);

    /// If needed, allow downcasting with `as_any()`.
    fn as_any(&self) -> &dyn Any;
}

/// A gRPC-based plugin connection implementation.
pub struct PluginGrpcConnection {
    info: Arc<tokio::sync::Mutex<PluginConnectionInfo>>,
    sender: UnboundedSender<PluginStreamResponse>,
}

impl PluginGrpcConnection {
    pub fn new(sender: UnboundedSender<PluginStreamResponse>, initially_enabled: bool) -> Self {
        let info = PluginConnectionInfo {
            name: "<uninitialized-grpc-plugin>".to_string(),
            capabilities: Vec::new(),
            is_enabled: initially_enabled,
        };
        Self {
            info: Arc::new(tokio::sync::Mutex::new(info)),
            sender,
        }
    }
}

#[async_trait]
impl PluginConnection for PluginGrpcConnection {
    async fn info(&self) -> PluginConnectionInfo {
        let guard = self.info.lock().await;
        guard.clone()
    }
    async fn set_capabilities(&self, capabilities: Vec<maowbot_proto::plugs::PluginCapability>) {
        let mut guard = self.info.lock().await;
        guard.capabilities = capabilities;
    }
    async fn set_name(&self, new_name: String) {
        let mut guard = self.info.lock().await;
        guard.name = new_name;
    }
    async fn send(&self, response: PluginStreamResponse) -> Result<(), Error> {
        self.sender
            .send(response)
            .map_err(|_| Error::Platform("Failed to send gRPC message".to_owned()))
    }
    async fn stop(&self) -> Result<(), Error> {
        let msg = PluginStreamResponse {
            payload: Some(RespPayload::ForceDisconnect(maowbot_proto::plugs::ForceDisconnect {
                reason: "Manager stopping connection".into(),
            })),
        };
        let _ = self.send(msg).await;
        Ok(())
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    async fn set_enabled(&self, enable: bool) {
        let mut guard = self.info.lock().await;
        guard.is_enabled = enable;
    }
}

/// An in-process plugin connection (e.g., loaded from a .so / .dll).
pub struct InProcessPluginConnection {
    plugin: Arc<dyn PluginConnection>,
    info: Arc<tokio::sync::Mutex<PluginConnectionInfo>>,
}

impl InProcessPluginConnection {
    pub fn new(plugin: Arc<dyn PluginConnection>, enabled: bool) -> Self {
        let info = PluginConnectionInfo {
            name: "<uninitialized-inproc-plugin>".to_string(),
            capabilities: Vec::new(),
            is_enabled: enabled,
        };
        Self {
            plugin,
            info: Arc::new(tokio::sync::Mutex::new(info)),
        }
    }
}

#[async_trait]
impl PluginConnection for InProcessPluginConnection {
    async fn info(&self) -> PluginConnectionInfo {
        let guard = self.info.lock().await;
        guard.clone()
    }
    async fn set_capabilities(&self, capabilities: Vec<maowbot_proto::plugs::PluginCapability>) {
        {
            let mut guard = self.info.lock().await;
            guard.capabilities = capabilities.clone();
        }
        self.plugin.set_capabilities(capabilities).await;
    }
    async fn set_name(&self, new_name: String) {
        {
            let mut guard = self.info.lock().await;
            guard.name = new_name.clone();
        }
        self.plugin.set_name(new_name).await;
    }
    async fn send(&self, response: PluginStreamResponse) -> Result<(), Error> {
        let guard = self.info.lock().await;
        if !guard.is_enabled {
            // If plugin is disabled, ignore
            return Ok(());
        }
        self.plugin.send(response).await
    }
    async fn stop(&self) -> Result<(), Error> {
        self.plugin.stop().await
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn set_bot_api(&self, api: Arc<dyn BotApi>) {
        self.plugin.set_bot_api(api);
    }
    async fn set_enabled(&self, enable: bool) {
        let mut guard = self.info.lock().await;
        guard.is_enabled = enable;
    }
}