// src/plugins/manager.rs

use super::protocol::{BotToPlugin, PluginToBot};
use super::capabilities::{PluginCapability, RequestedCapabilities, GrantedCapabilities};
use crate::Error;
use serde_json;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::net::TcpListener;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio_rustls::rustls::{Certificate, PrivateKey, ServerConfig}; // from tokio-rustls
use tokio_rustls::TlsAcceptor;
use tracing::{error, info};

/// Represents one connected plugin: either TCP-based or in-process dynamic library.
#[derive(Clone)]
pub struct PluginConnectionInfo {
    pub name: String,
    pub capabilities: Vec<PluginCapability>,
    // Could also store plugin’s numeric ID, version, etc.
}

/// This is the trait that both TCP-based plugins and dynamic plugins must implement.
#[async_trait::async_trait]
pub trait PluginConnection: Send + Sync {
    fn info(&self) -> PluginConnectionInfo;
    fn set_capabilities(&self, capabilities: Vec<PluginCapability>);

    /// Send a message from the bot to the plugin.
    fn send(&self, event: BotToPlugin) -> Result<(), Error>;

    /// Force a stop/shutdown of this plugin.
    async fn stop(&self) -> Result<(), Error>;
}

/// Concrete type for a TCP-based plugin.
pub struct TcpPluginConnection {
    /// Basic info about the plugin
    info: Arc<Mutex<PluginConnectionInfo>>,
    /// A channel for sending events to the plugin
    sender: tokio::sync::mpsc::UnboundedSender<BotToPlugin>,
}

impl TcpPluginConnection {
    pub fn new(name: String, sender: tokio::sync::mpsc::UnboundedSender<BotToPlugin>) -> Self {
        let info = PluginConnectionInfo {
            name,
            capabilities: Vec::new(),
        };
        TcpPluginConnection {
            info: Arc::new(Mutex::new(info)),
            sender,
        }
    }
}

#[async_trait::async_trait]
impl PluginConnection for TcpPluginConnection {
    fn info(&self) -> PluginConnectionInfo {
        self.info.lock().unwrap().clone()
    }

    fn set_capabilities(&self, capabilities: Vec<PluginCapability>) {
        self.info.lock().unwrap().capabilities = capabilities;
    }

    fn send(&self, event: BotToPlugin) -> Result<(), Error> {
        // Attempt to send the event over the mpsc channel
        self.sender
            .send(event)
            .map_err(|_| Error::Platform("Failed to send to plugin channel".into()))
    }

    async fn stop(&self) -> Result<(), Error> {
        // Possibly send a ForceDisconnect event, then drop the channel
        let _ = self.send(BotToPlugin::ForceDisconnect {
            reason: "Plugin manager stopping connection.".to_string()
        });
        Ok(())
    }
}

/// Example stub for an in-process plugin.
/// In a real system, you'd load the `.so`/`.dll`, find function pointers, etc.
pub struct DynamicPluginConnection {
    info: Arc<Mutex<PluginConnectionInfo>>,
    // For demonstration, we’ll store some placeholder for an FFI or function pointer.
    // Or we might store a channel to communicate with the loaded library code.
}

impl DynamicPluginConnection {
    pub fn load_dynamic_plugin(path: &str) -> Result<Self, Error> {
        // Here you would open the library using `libloading` or similar crate:
        // let lib = unsafe { libloading::Library::new(path)? };
        // Then find a symbol, e.g. `plugin_init`.
        // For now, we stub out:
        let info = PluginConnectionInfo {
            name: format!("dynamic_plugin_from_{}", path),
            capabilities: Vec::new(),
        };
        Ok(Self {
            info: Arc::new(Mutex::new(info)),
        })
    }
}

#[async_trait::async_trait]
impl PluginConnection for DynamicPluginConnection {
    fn info(&self) -> PluginConnectionInfo {
        self.info.lock().unwrap().clone()
    }

    fn set_capabilities(&self, capabilities: Vec<PluginCapability>) {
        self.info.lock().unwrap().capabilities = capabilities;
    }

    fn send(&self, event: BotToPlugin) -> Result<(), Error> {
        // If we had a function pointer or in-process callback, we’d call it here
        // For now, just log:
        info!("(InProcess) sending event to dynamic plugin: {:?}", event);
        Ok(())
    }

    async fn stop(&self) -> Result<(), Error> {
        info!("(InProcess) stopping dynamic plugin...");
        // Possibly call an FFI “plugin_shutdown” function pointer
        Ok(())
    }
}

/// A struct to keep track of all active plugin connections.
#[derive(Clone)]
pub struct PluginManager {
    plugins: Arc<Mutex<Vec<Arc<dyn PluginConnection>>>>,
    passphrase: Option<String>, // If set, the plugin must match this passphrase
    start_time: std::time::Instant,
}

impl PluginManager {
    /// Create a new PluginManager.
    pub fn new(passphrase: Option<String>) -> Self {
        PluginManager {
            plugins: Arc::new(Mutex::new(Vec::new())),
            passphrase,
            start_time: std::time::Instant::now(),
        }
    }

    /// Add a dynamic plugin (in-process) by loading a shared library or DLL.
    pub fn load_in_process_plugin(&self, path: &str) -> Result<(), Error> {
        let dynamic = DynamicPluginConnection::load_dynamic_plugin(path)?;
        self.plugins.lock().unwrap().push(Arc::new(dynamic));
        Ok(())
    }

    /// Start listening on the given address in plaintext (no TLS).
    pub async fn listen(&self, addr: &str) -> Result<(), Error> {
        let listener = TcpListener::bind(addr)
            .await
            .map_err(|e| Error::Platform(format!("Failed to bind: {}", e)))?;
        info!("PluginManager (plaintext) listening on {}", addr);

        loop {
            match listener.accept().await {
                Ok((stream, _)) => {
                    let manager = self.clone();
                    tokio::spawn(async move {
                        if let Err(e) = manager.handle_tcp_connection(stream).await {
                            error!("Plugin connection error: {:?}", e);
                        }
                    });
                }
                Err(e) => {
                    error!("Failed to accept plugin connection: {:?}", e);
                }
            }
        }
    }

    /// Alternatively, start listening using TLS.
    pub async fn listen_secure(&self, addr: &str, cert_path: &str, key_path: &str) -> Result<(), Error> {
        let certs = load_certs(cert_path)?;
        let key = load_key(key_path)?;

        let mut cfg = ServerConfig::builder()
            .with_safe_defaults()
            .with_no_client_auth()
            .with_single_cert(certs, key)
            .map_err(|e| Error::Platform(format!("TLS config error: {:?}", e)))?;

        cfg.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];

        let acceptor = TlsAcceptor::from(Arc::new(cfg));
        let listener = TcpListener::bind(addr)
            .await
            .map_err(|e| Error::Platform(format!("Failed to bind: {}", e)))?;

        info!("PluginManager (TLS) listening on {}", addr);

        loop {
            match listener.accept().await {
                Ok((socket, _)) => {
                    let manager = self.clone();
                    let acceptor_cloned = acceptor.clone();
                    tokio::spawn(async move {
                        match acceptor_cloned.accept(socket).await {
                            Ok(tls_stream) => {
                                if let Err(e) = manager.handle_tcp_connection(tls_stream).await {
                                    error!("Plugin connection (TLS) error: {:?}", e);
                                }
                            }
                            Err(e) => error!("TLS handshake error: {:?}", e),
                        }
                    });
                }
                Err(e) => {
                    error!("Failed to accept TLS plugin connection: {:?}", e);
                }
            }
        }
    }

    /// Internal method to handle a single TCP-based connection (plaintext or TLS).
    async fn handle_tcp_connection<T>(&self, stream: T) -> Result<(), Error>
    where
        T: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static,
    {
        let (reader, mut writer) = tokio::io::split(stream);
        let mut lines = BufReader::new(reader).lines();

        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<BotToPlugin>();

        // We read the very first line expecting a Hello message
        let line = match lines.next_line().await {
            Ok(Some(line)) => line,
            _ => {
                error!("No data received from plugin, closing.");
                return Ok(());
            }
        };

        let hello = match serde_json::from_str::<PluginToBot>(&line) {
            Ok(PluginToBot::Hello { plugin_name, passphrase }) => {
                if let Some(req) = &self.passphrase {
                    if Some(req.clone()) != passphrase {
                        let err_msg = BotToPlugin::AuthError {
                            reason: "Invalid passphrase".into(),
                        };
                        let out = serde_json::to_string(&err_msg)? + "\n";
                        writer.write_all(out.as_bytes()).await?;
                        error!("Plugin '{}' provided wrong passphrase!", plugin_name);
                        return Ok(()); // close connection
                    }
                }
                plugin_name
            }
            _ => {
                let err_msg = BotToPlugin::AuthError {
                    reason: "Expected Hello as first message".to_string(),
                };
                let out = serde_json::to_string(&err_msg)? + "\n";
                writer.write_all(out.as_bytes()).await?;
                error!("First message was not Hello. Closing.");
                return Ok(());
            }
        };

        // At this point, authentication succeeded; create a TCP plugin connection
        let tcp_plugin = TcpPluginConnection::new(hello.clone(), tx.clone());
        let plugin_arc = Arc::new(tcp_plugin);

        {
            let mut plugins = self.plugins.lock().unwrap();
            plugins.push(plugin_arc.clone());
        }

        // Immediately send a Welcome event
        let welcome = BotToPlugin::Welcome {
            bot_name: "MaowBot".to_string(),
        };
        let msg = serde_json::to_string(&welcome)? + "\n";
        writer.write_all(msg.as_bytes()).await?;

        // Spawn a read loop for inbound messages from the plugin
        let manager_clone = self.clone();
        let plugin_name_clone = hello.clone();
        tokio::spawn(async move {
            while let Ok(Some(line)) = lines.next_line().await {
                match serde_json::from_str::<PluginToBot>(&line) {
                    Ok(msg) => {
                        manager_clone.on_plugin_message(msg, &plugin_name_clone, plugin_arc.clone()).await;
                    }
                    Err(e) => {
                        error!("Invalid JSON from plugin {}: {} -- line: {}", plugin_name_clone, e, line);
                    }
                }
            }
            info!("Plugin '{}' read loop ended.", plugin_name_clone);
            // Remove from manager
            let mut plugins = manager_clone.plugins.lock().unwrap();
            if let Some(idx) = plugins.iter().position(|p| p.info().name == plugin_name_clone) {
                plugins.remove(idx);
            }
        });

        // Write loop: forward BotToPlugin events from rx to the plugin
        tokio::spawn(async move {
            while let Some(evt) = rx.recv().await {
                let out = serde_json::to_string(&evt).unwrap_or_else(|_| "{\"error\":\"serialize\"}".to_string());
                if let Err(e) = writer.write_all((out + "\n").as_bytes()).await {
                    error!("Error writing to plugin: {:?}", e);
                    break;
                }
            }
            info!("Plugin '{}' write loop ended.", hello);
        });

        Ok(())
    }

    /// Called whenever a plugin sends a `PluginToBot` message to the bot.
    async fn on_plugin_message(
        &self,
        message: PluginToBot,
        plugin_name: &str,
        plugin_conn: Arc<dyn PluginConnection>,
    ) {
        match message {
            PluginToBot::LogMessage { text } => {
                info!("[PLUGIN LOG: {}] {}", plugin_name, text);
            }
            PluginToBot::RequestStatus => {
                info!("Plugin '{}' requests status summary.", plugin_name);
                let status = self.build_status_response();
                let _ = plugin_conn.send(status);
            }
            PluginToBot::Shutdown => {
                info!("Plugin '{}' requests a bot shutdown. (Not implemented here).", plugin_name);
                // Could either comply or ignore.
            }
            PluginToBot::SwitchScene { scene_name } => {
                // First check if plugin has SceneManagement capability
                if plugin_conn.info().capabilities.contains(&PluginCapability::SceneManagement) {
                    info!("Plugin '{}' requests scene switch to: {}", plugin_name, scene_name);
                    // do scene switching here
                } else {
                    let err = BotToPlugin::AuthError {
                        reason: "You do not have SceneManagement capability.".to_string(),
                    };
                    let _ = plugin_conn.send(err);
                }
            }
            PluginToBot::SendChat { channel, text } => {
                if plugin_conn.info().capabilities.contains(&PluginCapability::SendChat) {
                    info!("(PLUGIN REQUEST: {}) SendChat to {}: {}", plugin_name, channel, text);
                    // Implement your chat-sending logic
                } else {
                    let err = BotToPlugin::AuthError {
                        reason: "You do not have SendChat capability.".to_string(),
                    };
                    let _ = plugin_conn.send(err);
                }
            }
            PluginToBot::RequestCapabilities(requested) => {
                info!("Plugin '{}' requests capabilities: {:?}", plugin_name, requested.requested);
                let (granted, denied) = self.evaluate_capabilities(&requested.requested);
                plugin_conn.set_capabilities(granted.clone());

                let response = BotToPlugin::CapabilityResponse(GrantedCapabilities {
                    granted,
                    denied,
                });
                let _ = plugin_conn.send(response);
            }
            PluginToBot::Hello { .. } => {
                // We already handled Hello as the first message.
                error!("Plugin '{}' sent Hello again unexpectedly.", plugin_name);
            }
        }
    }

    /// Simple logic to decide which capabilities we can grant to the plugin.
    fn evaluate_capabilities(&self, requested: &[PluginCapability]) -> (Vec<PluginCapability>, Vec<PluginCapability>) {
        // In a real scenario, you might consult config files, user roles, etc.
        // For demonstration: we grant everything except ChatModeration.
        let mut granted = vec![];
        let mut denied = vec![];
        for cap in requested {
            if *cap == PluginCapability::ChatModeration {
                denied.push(cap.clone());
            } else {
                granted.push(cap.clone());
            }
        }
        (granted, denied)
    }

    /// Broadcast an event to ALL connected plugins
    pub fn broadcast(&self, event: BotToPlugin) {
        let plugins = self.plugins.lock().unwrap();
        for plugin_conn in plugins.iter() {
            let _ = plugin_conn.send(event.clone());
        }
    }

    /// Example method: build a StatusResponse message
    fn build_status_response(&self) -> BotToPlugin {
        let plugins = self.plugins.lock().unwrap();
        let connected: Vec<String> = plugins.iter().map(|p| p.info().name.clone()).collect();
        let uptime_seconds = self.start_time.elapsed().as_secs();

        BotToPlugin::StatusResponse {
            connected_plugins: connected,
            server_uptime: uptime_seconds,
        }
    }
}

// If you're loading certs/keys from files:
fn load_certs(path: &str) -> Result<Vec<Certificate>, Error> {
    use rustls_pemfile::certs;
    use std::fs::File;
    use std::io::BufReader;

    let certfile = File::open(path)?;
    let mut reader = BufReader::new(certfile);

    let certs_raw = certs(&mut reader).map_err(|_| Error::Platform("Failed to read certs".into()))?;
    let mut certs_vec = Vec::new();
    for c in certs_raw {
        certs_vec.push(Certificate(c));
    }
    Ok(certs_vec)
}

fn load_key(path: &str) -> Result<PrivateKey, Error> {
    use rustls_pemfile::{pkcs8_private_keys, rsa_private_keys};
    use std::fs::File;
    use std::io::BufReader;

    let keyfile = File::open(path)?;
    let mut reader = BufReader::new(keyfile);

    // Try pkcs8
    if let Ok(keys) = pkcs8_private_keys(&mut reader) {
        if !keys.is_empty() {
            return Ok(PrivateKey(keys[0].clone()));
        }
    }

    // If that fails, rewind, try RSA
    let keyfile = File::open(path)?;
    let mut reader = BufReader::new(keyfile);
    if let Ok(keys) = rsa_private_keys(&mut reader) {
        if !keys.is_empty() {
            return Ok(PrivateKey(keys[0].clone()));
        }
    }

    Err(Error::Platform("No valid private key found".into()))
}
