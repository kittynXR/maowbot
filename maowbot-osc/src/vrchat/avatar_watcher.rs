// File: maowbot-osc/src/vrchat/avatar_watcher.rs

use std::{
    collections::HashMap,
    net::UdpSocket,
    path::PathBuf,
    sync::Arc,
    thread,
    time::Duration,
};
use tokio::sync::{Mutex, mpsc};
use notify::{
    event::{EventKind, RemoveKind, ModifyKind, CreateKind},
    Config, Event, RecommendedWatcher, RecursiveMode, Watcher,
};
use rosc::{OscPacket, OscMessage, OscType};
use rosc::decoder::decode_udp;
use crate::{OscError, Result};
use crate::vrchat::{parse_vrchat_avatar_config, VrchatAvatarConfig};
use crate::vrchat::toggles::avatar_toggle_menu::AvatarToggleMenu;

/// A single known avatar's JSON config from disk.
#[derive(Debug, Clone)]
pub struct KnownAvatar {
    pub path: PathBuf,
    pub config: VrchatAvatarConfig,
}

/// Represents the watcher that monitors `.json` files in a VRChat Avatars folder,
/// and also listens for `/avatar/change` OSC messages on UDP:9001.
pub struct AvatarWatcher {
    folder: PathBuf,
    // Maps "avatar_id" -> KnownAvatar
    known_avatars: HashMap<String, KnownAvatar>,
    changes_tx: mpsc::UnboundedSender<FileChangeEvent>,
    changes_rx: Option<mpsc::UnboundedReceiver<FileChangeEvent>>,
    is_running: bool,
}

impl AvatarWatcher {
    /// Creates a new `AvatarWatcher` that will watch the given folder.
    pub fn new(folder: PathBuf) -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        Self {
            folder,
            known_avatars: HashMap::new(),
            changes_tx: tx,
            changes_rx: Some(rx),
            is_running: false,
        }
    }

    /// Start watching the folder for JSON changes and spawn an OSC listener for `/avatar/change`.
    /// This uses a background thread (for file events) and a separate thread for OSC.
    pub fn start(&mut self) -> Result<()> {
        if self.is_running {
            return Ok(());
        }
        self.is_running = true;

        // 1) Initial scan
        self.reload_all_avatars()?;

        // 2) File watcher in a background thread
        let folder_clone = self.folder.clone();
        let changes_tx = self.changes_tx.clone();
        thread::spawn(move || {
            // Synchronous side of notify
            let (watch_send, watch_recv) = std::sync::mpsc::channel();

            // Create the watcher using notify's new 5.x API
            let mut watcher = match RecommendedWatcher::new(
                move |res: std::result::Result<Event, notify::Error>| {
                    match res {
                        Ok(event) => {
                            // Forward event
                            let _ = watch_send.send(event);
                        }
                        Err(e) => {
                            eprintln!("[AvatarWatcher] notify error: {:?}", e);
                        }
                    }
                },
                Config::default()
            ) {
                Ok(w) => w,
                Err(e) => {
                    eprintln!("[AvatarWatcher] Failed to create watcher: {:?}", e);
                    return;
                }
            };

            if let Err(e) = watcher.watch(&folder_clone, RecursiveMode::NonRecursive) {
                eprintln!("[AvatarWatcher] Watch error: {:?}", e);
                return;
            }

            // Relay file events
            loop {
                match watch_recv.recv() {
                    Ok(event) => {
                        let change_evt = FileChangeEvent::new(event);
                        let _ = changes_tx.send(change_evt);
                    }
                    Err(_) => break, // Channel closed
                }
            }
        });

        // 3) Start an async task that processes the file events.
        let known_map_ptr = Arc::new(Mutex::new(self.known_avatars.clone()));
        let mut local_rx = self.changes_rx.take().unwrap();
        let known_map_ptr_files = known_map_ptr.clone();
        tokio::spawn(async move {
            while let Some(evt) = local_rx.recv().await {
                match evt {
                    FileChangeEvent::Added(path) => {
                        println!("++ Added: {}", path.display());
                        maybe_parse_avatar(&path, &known_map_ptr_files).await;
                    }
                    FileChangeEvent::Modified(path) => {
                        println!("~~ Modified: {}", path.display());
                        maybe_parse_avatar(&path, &known_map_ptr_files).await;
                    }
                    FileChangeEvent::Removed(path) => {
                        println!("-- Removed: {}", path.display());
                        let mut guard = known_map_ptr_files.lock().await;
                        guard.retain(|_k, v| v.path != path);
                    }
                    FileChangeEvent::Other(e) => {
                        println!("[Watcher] Other event: {:?}", e);
                    }
                }
            }
        });

        // 4) Start a thread for OSC listening on UDP port 9001
        let known_map_ptr_osc = known_map_ptr.clone();
        thread::spawn(move || {
            let sock = match UdpSocket::bind("127.0.0.1:9001") {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("[AvatarWatcher] Could not bind OSC UDP port 9001: {:?}", e);
                    return;
                }
            };
            let mut buf = [0u8; 1024];
            println!("[AvatarWatcher] Listening for OSC at 127.0.0.1:9001 ...");
            loop {
                match sock.recv_from(&mut buf) {
                    Ok((size, _addr)) => {
                        match decode_udp(&buf[..size]) {
                            Ok((_remaining, packet)) => {
                                if let Err(e) = handle_osc_packet(packet, known_map_ptr_osc.clone()) {
                                    eprintln!("[AvatarWatcher] handle_osc_packet error: {:?}", e);
                                }
                            }
                            Err(e) => {
                                eprintln!("[AvatarWatcher] decode_udp error: {:?}", e);
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("[AvatarWatcher] recv_from error: {:?}", e);
                        thread::sleep(Duration::from_secs(1));
                    }
                }
            }
        });

        Ok(())
    }

    /// Reload all `.json` files from the folder into `known_avatars`.
    fn reload_all_avatars(&mut self) -> Result<()> {
        self.known_avatars.clear();

        if !self.folder.exists() {
            println!("[AvatarWatcher] Folder not found: {}", self.folder.display());
            return Ok(());
        }
        let entries = std::fs::read_dir(&self.folder)
            .map_err(|e| OscError::AvatarConfigError(format!("Unable to read dir: {:?}", e)))?;

        for entry in entries {
            if let Ok(de) = entry {
                let p = de.path();
                if p.extension().map(|ext| ext == "json").unwrap_or(false) {
                    match parse_vrchat_avatar_config(&p) {
                        Ok(cfg) => {
                            let av_id = cfg.id.clone();
                            let known = KnownAvatar { path: p.clone(), config: cfg };
                            self.known_avatars.insert(av_id, known);
                        }
                        Err(e) => {
                            eprintln!("[AvatarWatcher] Failed to parse {}: {}", p.display(), e);
                        }
                    }
                }
            }
        }

        println!("[AvatarWatcher] Loaded {} avatar configs from '{}'.",
                 self.known_avatars.len(),
                 self.folder.display()
        );
        Ok(())
    }
}

#[derive(Debug)]
pub enum FileChangeEvent {
    Added(PathBuf),
    Modified(PathBuf),
    Removed(PathBuf),
    Other(Event),
}

impl FileChangeEvent {
    pub fn new(event: Event) -> Self {
        match event.kind {
            EventKind::Create(CreateKind::File) => {
                if let Some(path) = event.paths.get(0) {
                    FileChangeEvent::Added(path.clone())
                } else {
                    FileChangeEvent::Other(event)
                }
            }
            EventKind::Modify(ModifyKind::Data(_)) => {
                if let Some(path) = event.paths.get(0) {
                    FileChangeEvent::Modified(path.clone())
                } else {
                    FileChangeEvent::Other(event)
                }
            }
            EventKind::Remove(RemoveKind::File) => {
                if let Some(path) = event.paths.get(0) {
                    FileChangeEvent::Removed(path.clone())
                } else {
                    FileChangeEvent::Other(event)
                }
            }
            _ => FileChangeEvent::Other(event),
        }
    }
}

/// Attempts to parse the avatar JSON at `path` and store it in the shared map.
async fn maybe_parse_avatar(path: &PathBuf, known_map_ptr: &Arc<Mutex<HashMap<String, KnownAvatar>>>) {
    if !path.exists() {
        return;
    }
    if path.extension().map(|ext| ext != "json").unwrap_or(true) {
        return;
    }
    match parse_vrchat_avatar_config(path) {
        Ok(cfg) => {
            println!("Parsed avatar config => id='{}', name='{}'", cfg.id, cfg.name);
            let mut guard = known_map_ptr.lock().await;
            guard.insert(cfg.id.clone(), KnownAvatar {
                path: path.clone(),
                config: cfg,
            });
        }
        Err(e) => {
            eprintln!("Failed to parse {}: {}", path.display(), e);
        }
    }
}

/// Handles an OSC packet and looks for `/avatar/change` messages.
fn handle_osc_packet(packet: OscPacket, known_map_ptr: Arc<Mutex<HashMap<String, KnownAvatar>>>) -> Result<()> {
    match packet {
        OscPacket::Message(msg) => {
            if msg.addr == "/avatar/change" {
                if let Some(arg) = msg.args.get(0) {
                    if let OscType::String(avatar_id_str) = arg {
                        let avatar_id = avatar_id_str.clone();
                        println!("[AvatarWatcher] DETECTED AVATAR CHANGE => {}", avatar_id);
                        tokio::spawn(async move {
                            let map_lock = known_map_ptr.lock().await;
                            if let Some(kav) = map_lock.get(&avatar_id) {
                                let mut menu = AvatarToggleMenu::new(&kav.config);
                                menu.print_menu();
                            } else {
                                eprintln!("No local config for avatar_id={}", avatar_id);
                            }
                        });
                    }
                }
            }
        }
        OscPacket::Bundle(bundle) => {
            for inner in bundle.content {
                handle_osc_packet(inner, known_map_ptr.clone())?;
            }
        }
    }
    Ok(())
}
