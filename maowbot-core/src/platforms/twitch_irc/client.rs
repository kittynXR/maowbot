use std::io;
use std::sync::Arc;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter, split};
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use tokio_native_tls::native_tls;
use tokio_native_tls::TlsConnector;
use tracing::{info, warn, error, debug, trace};

/// Minimal representation of a parsed IRC message from Twitch.
#[derive(Debug, Clone)]
pub struct ParsedTwitchMsg {
    pub tags: Option<String>,
    pub prefix: Option<String>,
    pub command: String,
    pub params: Vec<String>,
    pub trailing: Option<String>,
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

/// Parse user roles from the IRC tags
fn parse_twitch_roles(tags: &str) -> Vec<String> {
    let mut roles = Vec::new();

    if let Some(badges) = extract_tag_value(tags, "badges") {
        // e.g. "broadcaster/1,subscriber/0"
        for part in badges.split(',') {
            if let Some((badge, _lvl)) = part.split_once('/') {
                roles.push(badge.to_string());
            }
        }
    }
    if let Some(m) = extract_tag_value(tags, "mod") {
        if m == "1" {
            roles.push("mod".to_string());
        }
    }
    if let Some(v) = extract_tag_value(tags, "vip") {
        if v == "1" {
            roles.push("vip".to_string());
        }
    }
    if let Some(s) = extract_tag_value(tags, "subscriber") {
        if s == "1" {
            roles.push("subscriber".to_string());
        }
    }

    roles
}

impl ParsedTwitchMsg {
    pub fn parse_irc_line(line: &str) -> Self {
        let mut rest = line.trim();
        let mut tags = None;
        let mut prefix = None;
        let mut command = String::new();
        let mut params = Vec::new();
        let mut trailing = None;

        // 1) extract tags if line starts with '@'
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

        // 2) extract prefix if line starts with ':'
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

        // 3) The next token is the command
        let mut parts = rest.splitn(2, ' ');
        if let Some(cmd) = parts.next() {
            command = cmd.to_string();
        }
        rest = parts.next().unwrap_or("");

        // 4) Attempt to find the trailing portion after " :"
        if rest.starts_with(':') {
            trailing = Some(rest.trim_start_matches(':').to_string());
        } else if let Some(idx) = rest.find(" :") {
            trailing = Some(rest[idx + 2..].to_string());
            let before = rest[..idx].trim();
            if !before.is_empty() {
                params.extend(before.split_whitespace().map(|s| s.to_string()));
            }
        } else {
            // no " :", so treat everything as normal params
            params.extend(rest.split_whitespace().map(|s| s.to_string()));
        }

        Self {
            tags,
            prefix,
            command,
            params,
            trailing,
        }
    }
}

/// A higher-level event from the IRC read loop.
#[derive(Debug, Clone)]
pub struct IrcIncomingEvent {
    /// The real numeric user ID from `user-id=...` in the tags. This is our unique key in DB:
    pub twitch_user_id: Option<String>,

    /// The user’s display name if present in tags, or fallback from prefix
    pub display_name: Option<String>,

    pub channel: Option<String>,
    pub text: Option<String>,
    pub raw_line: String,
    pub command: String,
    pub roles: Vec<String>,
}

pub struct TwitchIrcClient {
    pub incoming: Option<mpsc::UnboundedReceiver<IrcIncomingEvent>>,
    raw_outgoing: mpsc::UnboundedSender<String>,

    read_task: JoinHandle<()>,
    write_task: JoinHandle<()>,
}

impl TwitchIrcClient {
    pub async fn connect(username: &str, oauth_token: &str) -> io::Result<Self> {
        let tcp = TcpStream::connect(("irc.chat.twitch.tv", 6697)).await
            .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("TCP connect error: {e}")))?;

        let native_connector = native_tls::TlsConnector::new()
            .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("TLSConnector::new() => {e}")))?;
        let connector = TlsConnector::from(native_connector);

        let domain = "irc.chat.twitch.tv";
        let tls_stream = connector.connect(domain, tcp).await
            .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("TLS connect() => {e}")))?;

        let (read_half, write_half) = split(tls_stream);

        let (tx_outgoing, rx_outgoing) = mpsc::unbounded_channel::<String>();
        let (tx_incoming, rx_incoming) = mpsc::unbounded_channel::<IrcIncomingEvent>();

        let write_task = tokio::spawn(Self::writer_loop(write_half, rx_outgoing));
        tx_outgoing.send(format!("PASS {}", oauth_token)).ok();
        tx_outgoing.send(format!("NICK {}", username)).ok();
        tx_outgoing
            .send("CAP REQ :twitch.tv/commands twitch.tv/tags twitch.tv/membership".to_string())
            .ok();

        let read_task = tokio::spawn(Self::reader_loop(read_half, tx_incoming.clone(), tx_outgoing.clone()));

        Ok(Self {
            incoming: Some(rx_incoming),
            raw_outgoing: tx_outgoing,
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
                            trace!("Auto PONG -> {}", trail);
                        }
                        continue;
                    }

                    let mut evt = IrcIncomingEvent {
                        twitch_user_id: None,
                        display_name: None,
                        channel: None,
                        text: None,
                        raw_line: line.clone(),
                        command: command.clone(),
                        roles: vec![],
                    };

                    if command == "PRIVMSG" {
                        // Usually in parsed.params[0] is "#channel"
                        if let Some(ch) = parsed.params.get(0) {
                            evt.channel = Some(ch.clone());
                        }
                        evt.text = parsed.trailing.clone();

                        if let Some(tags) = &parsed.tags {
                            // parse user-id, display-name, roles
                            if let Some(uid) = extract_tag_value(tags, "user-id") {
                                evt.twitch_user_id = Some(uid);
                            }
                            if let Some(dn) = extract_tag_value(tags, "display-name") {
                                evt.display_name = Some(dn);
                            }
                            evt.roles = parse_twitch_roles(tags);
                        }
                        else if let Some(pref) = &parsed.prefix {
                            // fallback for username in prefix
                            if let Some(excl) = pref.find('!') {
                                let fallback_name = &pref[..excl];
                                evt.display_name = Some(fallback_name.to_string());
                            }
                        }
                    }
                    else if command == "JOIN" || command == "PART" {
                        // channel is in params[0], name is from prefix or display-name in tags
                        if let Some(ch) = parsed.params.get(0) {
                            evt.channel = Some(ch.clone());
                        }
                        if let Some(tags) = &parsed.tags {
                            evt.roles = parse_twitch_roles(tags);
                            if let Some(uid) = extract_tag_value(tags, "user-id") {
                                evt.twitch_user_id = Some(uid);
                            }
                            if let Some(dn) = extract_tag_value(tags, "display-name") {
                                evt.display_name = Some(dn);
                            }
                        }
                        else if let Some(pref) = &parsed.prefix {
                            if let Some(excl) = pref.find('!') {
                                let fallback_name = &pref[..excl];
                                evt.display_name = Some(fallback_name.to_string());
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
            if writer.write_all(line.as_bytes()).await.is_err() {
                break;
            }
            if writer.write_all(b"\r\n").await.is_err() {
                break;
            }
            if writer.flush().await.is_err() {
                break;
            }
        }
        info!("(TwitchIrcClient) writer_loop ended.");
    }

    pub fn join_channel(&self, channel: &str) {
        let _ = self.raw_outgoing.send(format!("JOIN {}", channel));
    }

    pub fn part_channel(&self, channel: &str) {
        let _ = self.raw_outgoing.send(format!("PART {}", channel));
    }

    pub fn send_privmsg(&self, channel: &str, message: &str) {
        let cmd = format!("PRIVMSG {} :{}", channel, message);
        let _ = self.raw_outgoing.send(cmd);
    }

    pub fn shutdown(self) {
        self.read_task.abort();
        self.write_task.abort();
    }
}
