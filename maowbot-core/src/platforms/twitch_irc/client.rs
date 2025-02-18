//! src/platforms/twitch_irc/client.rs

use std::io;
use std::sync::Arc;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter, split};
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use tokio_native_tls::native_tls;
use tokio_native_tls::TlsConnector;
use tracing::{info, warn, error, debug};

/// Minimal representation of a parsed IRC message from Twitch.
#[derive(Debug, Clone)]
pub struct ParsedTwitchMsg {
    pub tags: Option<String>,
    pub prefix: Option<String>,
    pub command: String,
    pub params: Vec<String>,
    pub trailing: Option<String>,
}

impl ParsedTwitchMsg {
    pub fn parse_irc_line(line: &str) -> Self {
        let mut rest = line.trim();
        let mut tags = None;
        let mut prefix = None;
        let mut command = String::new();
        let mut params = Vec::new();
        let mut trailing = None;

        // 1) extract tags
        if rest.starts_with('@') {
            if let Some(space_pos) = rest.find(' ') {
                tags = Some(rest[..space_pos].to_string());
                rest = &rest[space_pos + 1..];
            } else {
                return Self {
                    tags: Some(rest.to_string()),
                    prefix: None,
                    command,
                    params,
                    trailing,
                };
            }
        }

        // 2) extract prefix
        if rest.starts_with(':') {
            if let Some(space_pos) = rest.find(' ') {
                prefix = Some(rest[..space_pos].trim_start_matches(':').to_string());
                rest = &rest[space_pos + 1..];
            } else {
                return Self {
                    tags,
                    prefix: Some(rest.trim_start_matches(':').to_string()),
                    command,
                    params,
                    trailing,
                };
            }
        }

        // 3) command
        let mut parts = rest.splitn(2, ' ');
        if let Some(cmd) = parts.next() {
            command = cmd.to_string();
        }
        rest = parts.next().unwrap_or("");

        // 4) check for trailing
        if let Some(idx) = rest.find(" :") {
            // trailing after " :"
            trailing = Some(rest[idx + 2..].to_string());
            let before = rest[..idx].trim();
            if !before.is_empty() {
                params.extend(before.split_whitespace().map(|s| s.to_string()));
            }
        } else {
            // all params
            params.extend(rest.split_whitespace().map(|s| s.to_string()));
        }

        Self { tags, prefix, command, params, trailing }
    }
}

/// Higher-level event from the IRC read loop.
#[derive(Debug, Clone)]
pub struct IrcIncomingEvent {
    pub channel: Option<String>,
    pub user_name: Option<String>,
    pub user_id: Option<String>,
    pub text: Option<String>,
    pub raw_line: String,
    pub command: String,
}

/// Low-level IRC client that connects to Twitch via TLS.
pub struct TwitchIrcClient {
    /// For sending raw lines out:
    raw_outgoing: mpsc::UnboundedSender<String>,

    /// We store the incoming event channel as an **Option** so we can `take()` if needed.
    pub incoming: Option<mpsc::UnboundedReceiver<IrcIncomingEvent>>,

    read_task: JoinHandle<()>,
    write_task: JoinHandle<()>,
}

impl TwitchIrcClient {
    /// Tries to connect to `irc.chat.twitch.tv:6697` with TLS, does PASS/NICK,
    /// spawns read/write tasks, and returns a `TwitchIrcClient`.
    pub async fn connect(
        username: &str,
        oauth_token: &str,
    ) -> io::Result<Self> {
        // 1) raw TCP connect
        let tcp = TcpStream::connect(("irc.chat.twitch.tv", 6697))
            .await
            .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("TCP connect error: {e}")))?;

        // 2) TLS handshake
        let native_connector = native_tls::TlsConnector::new()
            .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("TLSConnector::new() => {e}")))?;
        let connector = TlsConnector::from(native_connector);

        let domain = "irc.chat.twitch.tv";
        let tls_stream = connector.connect(domain, tcp).await
            .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("TLS connect() => {e}")))?;

        let (read_half, write_half) = split(tls_stream);

        // 3) channels
        let (tx_outgoing, rx_outgoing) = mpsc::unbounded_channel::<String>();
        let (tx_incoming, rx_incoming) = mpsc::unbounded_channel::<IrcIncomingEvent>();

        // 4) spawn writer
        let write_task = tokio::spawn(Self::writer_loop(write_half, rx_outgoing));

        // send PASS/NICK/CAP:
        tx_outgoing.send(format!("PASS {}", oauth_token)).ok();
        tx_outgoing.send(format!("NICK {}", username)).ok();
        tx_outgoing.send("CAP REQ :twitch.tv/commands twitch.tv/tags twitch.tv/membership".to_string()).ok();

        // 5) spawn reader
        let read_task = tokio::spawn(Self::reader_loop(
            read_half,
            tx_incoming.clone(),
            tx_outgoing.clone(),
        ));

        Ok(Self {
            raw_outgoing: tx_outgoing,
            incoming: Some(rx_incoming),
            read_task,
            write_task,
        })
    }

    async fn reader_loop<R>(
        read_half: R,
        tx_incoming: mpsc::UnboundedSender<IrcIncomingEvent>,
        tx_outgoing: mpsc::UnboundedSender<String>,
    )
    where
        R: tokio::io::AsyncRead + Unpin,
    {
        let mut reader = BufReader::new(read_half);
        let mut line_buffer = String::new();

        loop {
            line_buffer.clear();
            match reader.read_line(&mut line_buffer).await {
                Ok(0) => {
                    // EOF
                    info!("(TwitchIrcClient) read_loop => EOF");
                    break;
                }
                Ok(_) => {
                    let line = line_buffer.trim_end().to_string();
                    if line.is_empty() {
                        continue;
                    }
                    debug!("<< {}", line);

                    let parsed = ParsedTwitchMsg::parse_irc_line(&line);
                    let command = parsed.command.to_uppercase();

                    // respond to PING
                    if command == "PING" {
                        if let Some(trail) = parsed.trailing {
                            tx_outgoing.send(format!("PONG :{}", trail)).ok();
                            debug!("Auto PONG -> {}", trail);
                        }
                        continue;
                    }

                    let mut evt = IrcIncomingEvent {
                        channel: None,
                        user_name: None,
                        user_id: None,
                        text: None,
                        raw_line: line.clone(),
                        command: command.clone(),
                    };

                    // if PRIVMSG
                    if command == "PRIVMSG" {
                        if let Some(ch) = parsed.params.get(0) {
                            evt.channel = Some(ch.clone());
                        }
                        evt.text = parsed.trailing.clone();

                        // user name from prefix
                        if let Some(ref prefix) = parsed.prefix {
                            if let Some(excl) = prefix.find('!') {
                                evt.user_name = Some(prefix[..excl].to_string());
                            }
                        }
                        // user id from tags
                        if let Some(ref t) = parsed.tags {
                            if let Some(uid) = extract_tag_value(t, "user-id") {
                                evt.user_id = Some(uid);
                            }
                            if let Some(dn) = extract_tag_value(t, "display-name") {
                                evt.user_name = Some(dn);
                            }
                        }
                    }
                    else if command == "JOIN" {
                        if let Some(ch) = parsed.params.get(0) {
                            evt.channel = Some(ch.clone());
                        }
                        if let Some(ref prefix) = parsed.prefix {
                            if let Some(excl) = prefix.find('!') {
                                evt.user_name = Some(prefix[..excl].to_string());
                            }
                        }
                    }
                    else if command == "PART" {
                        if let Some(ch) = parsed.params.get(0) {
                            evt.channel = Some(ch.clone());
                        }
                        if let Some(ref prefix) = parsed.prefix {
                            if let Some(excl) = prefix.find('!') {
                                evt.user_name = Some(prefix[..excl].to_string());
                            }
                        }
                    }

                    let _ = tx_incoming.send(evt);
                }
                Err(e) => {
                    error!("(TwitchIrcClient) read error => {:?}", e);
                    break;
                }
            }
        }

        info!("(TwitchIrcClient) reader_loop ended.");
    }

    async fn writer_loop<W>(
        mut write_half: W,
        mut rx_outgoing: mpsc::UnboundedReceiver<String>,
    )
    where
        W: tokio::io::AsyncWrite + Unpin,
    {
        let mut writer = BufWriter::new(&mut write_half);

        while let Some(line) = rx_outgoing.recv().await {
            debug!(">> {}", line);
            if let Err(e) = writer.write_all(line.as_bytes()).await {
                error!("writer error => {:?}", e);
                break;
            }
            if let Err(e) = writer.write_all(b"\r\n").await {
                error!("writer error => {:?}", e);
                break;
            }
            if let Err(e) = writer.flush().await {
                error!("writer flush error => {:?}", e);
                break;
            }
        }

        info!("(TwitchIrcClient) writer_loop ended.");
    }

    pub fn send_raw_line(&self, line: &str) {
        let _ = self.raw_outgoing.send(line.to_string());
    }

    pub fn join_channel(&self, channel: &str) {
        self.send_raw_line(&format!("JOIN {}", channel));
    }

    pub fn part_channel(&self, channel: &str) {
        self.send_raw_line(&format!("PART {}", channel));
    }

    pub fn send_privmsg(&self, channel: &str, message: &str) {
        let cmd = format!("PRIVMSG {} :{}", channel, message);
        self.send_raw_line(&cmd);
    }

    /// Aborts the read/write tasks by dropping channels and calling .abort().
    pub fn shutdown(self) {
        self.read_task.abort();
        self.write_task.abort();
    }
}

/// Helper to extract `key=value` from a tag string like `@badge-info=;user-id=1234;...`
fn extract_tag_value(tag_str: &str, key: &str) -> Option<String> {
    let kvpairs = tag_str.trim_start_matches('@').split(';');
    for kv in kvpairs {
        let mut parts = kv.splitn(2, '=');
        let left = parts.next().unwrap_or("");
        let right = parts.next().unwrap_or("");
        if left == key {
            return Some(right.to_string());
        }
    }
    None
}
