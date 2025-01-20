// src/plugins/manager.rs

use super::protocol::{BotToPlugin, PluginToBot};
use crate::Error;
use serde_json;
use std::sync::{Arc, Mutex};
use tokio::net::TcpListener;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio_rustls::rustls::{Certificate, PrivateKey, ServerConfig}; // <<== from tokio-rustls
use tokio_rustls::TlsAcceptor;
use tracing::{error, info};
use std::sync::Arc as StdArc;

/// Represents one connected plugin with a name + channel for sending messages.
#[derive(Clone)]
struct PluginConnection {
    name: String,
    sender: tokio::sync::mpsc::UnboundedSender<BotToPlugin>,
}

/// A struct to keep track of all active plugin connections.
#[derive(Clone)]
pub struct PluginManager {
    plugins: Arc<Mutex<Vec<PluginConnection>>>,
    passphrase: Option<String>, // If set, the plugin must match this passphrase
    start_time: std::time::Instant,
}

impl PluginManager {
    /// Create a new PluginManager.
    /// Optionally provide a passphrase for plugin authentication.
    pub fn new(passphrase: Option<String>) -> Self {
        PluginManager {
            plugins: Arc::new(Mutex::new(Vec::new())),
            passphrase,
            start_time: std::time::Instant::now(),
        }
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
                        if let Err(e) = manager.handle_connection(stream).await {
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

    /// Start listening using TLS (if you prefer encryption).
    /// Provide paths to a cert/key, or generate them at runtime.
    pub async fn listen_secure(&self, addr: &str, cert_path: &str, key_path: &str) -> Result<(), Error> {
        use tokio_rustls::rustls::{self, Certificate, PrivateKey, ServerConfig};
        use tokio_rustls::TlsAcceptor;
        use std::fs;
        use std::sync::Arc as StdArc;

        let certs = load_certs(cert_path)?;
        let key = load_key(key_path)?;

        let mut cfg = ServerConfig::builder()
            .with_safe_defaults()
            .with_no_client_auth()
            .with_single_cert(certs, key)
            .map_err(|e| Error::Platform(format!("TLS config error: {:?}", e)))?;

        // If you want to force TLS 1.2 or something, do it here
        cfg.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];

        let acceptor = TlsAcceptor::from(StdArc::new(cfg));
        let listener = TcpListener::bind(addr)
            .await
            .map_err(|e| Error::Platform(format!("Failed to bind: {}", e)))?;
        info!("PluginManager (TLS) listening on {}", addr);

        loop {
            let (socket, _) = listener
                .accept()
                .await
                .map_err(|e| Error::Platform(format!("TLS accept error: {}", e)))?;

            let manager = self.clone();
            let acceptor_cloned = acceptor.clone();
            tokio::spawn(async move {
                // Accept the TLS handshake
                match acceptor_cloned.accept(socket).await {
                    Ok(tls_stream) => {
                        if let Err(e) = manager.handle_connection(tls_stream).await {
                            error!("Plugin connection (TLS) error: {:?}", e);
                        }
                    }
                    Err(e) => error!("TLS handshake error: {:?}", e),
                }
            });
        }
    }

    /// Handle a single plugin connection (plaintext or TLS).
    async fn handle_connection<T>(&self, stream: T) -> Result<(), Error>
    where
        T: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static,
    {
        let (reader, mut writer) = tokio::io::split(stream);
        let mut lines = BufReader::new(reader).lines();

        // We haven't confirmed the plugin's passphrase yet, so we create a temp channel
        // but won't store it in self.plugins until we get "Hello { plugin_name, passphrase }".
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<BotToPlugin>();

        // We'll read the very first line(s) to see if it's the Hello message
        let line = match lines.next_line().await {
            Ok(Some(line)) => line,
            _ => {
                error!("No data received from plugin, closing.");
                return Ok(());
            }
        };

        let hello = match serde_json::from_str::<PluginToBot>(&line) {
            Ok(PluginToBot::Hello { plugin_name, passphrase }) => {
                // Check passphrase if needed
                if let Some(req) = &self.passphrase {
                    if Some(req.clone()) != passphrase {
                        // Mismatch
                        let err_msg = BotToPlugin::AuthError {
                            reason: "Invalid passphrase".into(),
                        };
                        let out = serde_json::to_string(&err_msg)? + "\n";
                        writer.write_all(out.as_bytes()).await?;
                        error!("Plugin '{}' provided wrong passphrase!", plugin_name);
                        // Close
                        return Ok(());
                    }
                }
                plugin_name
            }
            Ok(other_msg) => {
                // We expected Hello as the first message. If we got something else, reject.
                let err_msg = BotToPlugin::AuthError {
                    reason: "Expected Hello as first message".to_string(),
                };
                let out = serde_json::to_string(&err_msg)? + "\n";
                writer.write_all(out.as_bytes()).await?;
                error!("First message was not Hello. Closing.");
                return Ok(());
            }
            Err(e) => {
                error!("Failed to parse first message: {:?}", e);
                return Ok(());
            }
        };

        // At this point, authentication succeeded. We can store the plugin connection.
        {
            let mut plugins = self.plugins.lock().unwrap();
            plugins.push(PluginConnection {
                name: hello.clone(),
                sender: tx.clone(),
            });
        }

        // 1) Immediately send a welcome event
        let welcome = BotToPlugin::Welcome {
            bot_name: "MaowBot".to_string(),
        };
        let msg = serde_json::to_string(&welcome)? + "\n";
        writer.write_all(msg.as_bytes()).await?;

        // We'll spawn a read loop + a write loop

        // READ LOOP: handle all subsequent lines from plugin
        let manager_clone = self.clone();
        let plugin_name_clone = hello.clone();

        tokio::spawn(async move {
            let mut lines = lines; // move into this task
            while let Ok(Some(line)) = lines.next_line().await {
                match serde_json::from_str::<PluginToBot>(&line) {
                    Ok(msg) => {
                        manager_clone.on_plugin_message(msg, &plugin_name_clone, tx.clone()).await;
                    }
                    Err(e) => {
                        error!("Invalid JSON from plugin {}: {} -- line: {}", plugin_name_clone, e, line);
                    }
                }
            }
            info!("Plugin '{}' read loop ended.", plugin_name_clone);

            // If we get here, the plugin disconnected
            // Remove from the list
            let mut plugins = manager_clone.plugins.lock().unwrap();
            if let Some(idx) = plugins.iter().position(|p| p.name == plugin_name_clone) {
                plugins.remove(idx);
            }
        });

        // WRITE LOOP: forward BotToPlugin events from rx to the plugin
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
        tx: tokio::sync::mpsc::UnboundedSender<BotToPlugin>,
    ) {
        match message {
            PluginToBot::LogMessage { text } => {
                info!("[PLUGIN LOG: {}] {}", plugin_name, text);
            }
            PluginToBot::SendChat { channel, text } => {
                info!("(PLUGIN REQUEST: {}) SendChat to {}: {}", plugin_name, channel, text);
                // Here you'd call your normal chat-sending logic
            }
            PluginToBot::Hello { .. } => {
                // We already processed Hello as the first message, ignoring now
                error!("Plugin '{}' sent Hello again unexpectedly.", plugin_name);
            }
            PluginToBot::Shutdown => {
                info!("Plugin '{}' requests shutdown. (Not yet implemented)", plugin_name);
            }
            PluginToBot::RequestStatus => {
                info!("Plugin '{}' requests status summary.", plugin_name);
                let status = self.build_status_response();
                let _ = tx.send(status);
            }
            PluginToBot::SwitchScene { scene_name } => {
                info!("Plugin '{}' requests to switch scene: {}", plugin_name, scene_name);
                // Not implemented, but you can add your OBS or scene-switching logic here
            }
        }
    }

    /// Broadcast an event to ALL connected plugins, e.g. on new chat messages.
    pub fn broadcast(&self, event: BotToPlugin) {
        let plugins = self.plugins.lock().unwrap();
        for plugin_conn in plugins.iter() {
            let _ = plugin_conn.sender.send(event.clone());
        }
    }

    /// Example method: build a StatusResponse message
    fn build_status_response(&self) -> BotToPlugin {
        let plugins = self.plugins.lock().unwrap();
        let connected: Vec<String> = plugins.iter().map(|p| p.name.clone()).collect();
        let uptime_seconds = self.start_time.elapsed().as_secs();

        BotToPlugin::StatusResponse {
            connected_plugins: connected,
            server_uptime: uptime_seconds,
        }
    }
}

// If you're loading certs/keys from files:
fn load_certs(path: &str) -> Result<Vec<Certificate>, Error> {
    use rustls_pemfile::certs; // This crate is typically version-matched with rustls 0.20
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
