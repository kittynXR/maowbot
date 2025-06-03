use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use tokio::process::{Child, Command};
use tokio::sync::Mutex;
use tokio::io::{AsyncBufReadExt, BufReader};
use tracing::{info, error, debug, warn};
use crate::AppEvent;

#[derive(Debug, Clone, Copy)]
pub enum ProcessType {
    Server,
    Overlay,
}

#[derive(Debug, Clone)]
pub struct ProcessStatus {
    pub running: bool,
    pub pid: Option<u32>,
}

pub struct ProcessManager {
    server_process: Arc<Mutex<Option<Child>>>,
    overlay_process: Arc<Mutex<Option<Child>>>,
    event_tx: Option<crossbeam_channel::Sender<AppEvent>>,
}

impl ProcessManager {
    pub fn new() -> Self {
        Self {
            server_process: Arc::new(Mutex::new(None)),
            overlay_process: Arc::new(Mutex::new(None)),
            event_tx: None,
        }
    }

    pub fn with_event_sender(event_tx: crossbeam_channel::Sender<AppEvent>) -> Self {
        Self {
            server_process: Arc::new(Mutex::new(None)),
            overlay_process: Arc::new(Mutex::new(None)),
            event_tx: Some(event_tx),
        }
    }

    /// Check if a process is running
    pub async fn is_running(&self, process_type: ProcessType) -> bool {
        let process = match process_type {
            ProcessType::Server => &self.server_process,
            ProcessType::Overlay => &self.overlay_process,
        };

        let mut guard = process.lock().await;
        if let Some(child) = guard.as_mut() {
            match child.try_wait() {
                Ok(Some(_)) => {
                    // Process has exited
                    *guard = None;
                    false
                }
                Ok(None) => {
                    // Still running
                    true
                }
                Err(_) => false,
            }
        } else {
            false
        }
    }

    /// Get process status
    pub async fn get_status(&self, process_type: ProcessType) -> ProcessStatus {
        let process = match process_type {
            ProcessType::Server => &self.server_process,
            ProcessType::Overlay => &self.overlay_process,
        };

        let guard = process.lock().await;
        if let Some(child) = guard.as_ref() {
            ProcessStatus {
                running: true,
                pid: child.id(),
            }
        } else {
            ProcessStatus {
                running: false,
                pid: None,
            }
        }
    }

    /// Start the server process
    pub async fn start_server(&self) -> Result<(), Box<dyn std::error::Error>> {
        let mut guard = self.server_process.lock().await;
        
        if guard.is_some() {
            info!("Server already running");
            return Ok(());
        }

        info!("Starting maowbot-server...");

        // Try to find the server executable
        let exe_name = if cfg!(windows) { "maowbot-server.exe" } else { "maowbot-server" };
        
        let possible_paths = vec![
            // Same directory as current exe
            std::env::current_exe()?.parent().unwrap().join(exe_name),
            // Relative paths from working directory
            PathBuf::from(format!("./target/debug/{}", exe_name)),
            PathBuf::from(format!("./target/release/{}", exe_name)),
            PathBuf::from(format!("../maowbot-server/target/debug/{}", exe_name)),
            PathBuf::from(format!("../maowbot-server/target/release/{}", exe_name)),
        ];

        let server_path = possible_paths
            .into_iter()
            .find(|p| {
                let exists = p.exists();
                debug!("Checking server path: {:?} - exists: {}", p, exists);
                exists
            })
            .ok_or("Could not find maowbot-server executable")?;

        info!("Starting server from: {:?}", server_path);

        let mut cmd = Command::new(&server_path);
        cmd.stdout(Stdio::piped())
            .stderr(Stdio::piped());

        // Pass through any relevant environment variables
        if let Ok(log_level) = std::env::var("RUST_LOG") {
            cmd.env("RUST_LOG", log_level);
        }

        let mut child = cmd.spawn()?;
        let pid = child.id();
        info!("Server process started with PID: {:?}", pid);

        // Capture stdout
        if let Some(stdout) = child.stdout.take() {
            tokio::spawn(async move {
                let reader = BufReader::new(stdout);
                let mut lines = reader.lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    info!("[server] {}", line);
                }
            });
        }

        // Capture stderr
        if let Some(stderr) = child.stderr.take() {
            tokio::spawn(async move {
                let reader = BufReader::new(stderr);
                let mut lines = reader.lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    error!("[server] {}", line);
                }
            });
        }

        *guard = Some(child);
        drop(guard);

        // Wait for server to be ready by checking if we can connect
        info!("Waiting for server to be ready...");
        for i in 0..30 {
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
            
            // Check if process is still running
            if !self.is_running(ProcessType::Server).await {
                return Err("Server process exited unexpectedly".into());
            }

            // Try to connect to the gRPC port
            match tokio::net::TcpStream::connect("127.0.0.1:9999").await {
                Ok(_) => {
                    info!("Server is ready!");
                    return Ok(());
                }
                Err(_) => {
                    if i == 29 {
                        warn!("Server taking longer than expected to start, but continuing anyway");
                    }
                }
            }
        }

        Ok(())
    }

    /// Start the overlay process
    pub async fn start_overlay(&self) -> Result<(), Box<dyn std::error::Error>> {
        let mut guard = self.overlay_process.lock().await;
        
        if guard.is_some() {
            info!("Overlay already running");
            return Ok(());
        }

        info!("Starting maowbot-overlay...");

        // Try to find the overlay executable
        let exe_name = if cfg!(windows) { "maowbot-overlay.exe" } else { "maowbot-overlay" };
        
        let possible_paths = vec![
            // Same directory as current exe
            std::env::current_exe()?.parent().unwrap().join(exe_name),
            // Relative paths from working directory
            PathBuf::from(format!("./target/debug/{}", exe_name)),
            PathBuf::from(format!("./target/release/{}", exe_name)),
            PathBuf::from(format!("../maowbot-overlay/target/debug/{}", exe_name)),
            PathBuf::from(format!("../maowbot-overlay/target/release/{}", exe_name)),
        ];

        let overlay_path = possible_paths
            .into_iter()
            .find(|p| {
                let exists = p.exists();
                debug!("Checking overlay path: {:?} - exists: {}", p, exists);
                exists
            })
            .ok_or("Could not find maowbot-overlay executable")?;

        info!("Starting overlay from: {:?}", overlay_path);

        let mut cmd = Command::new(&overlay_path);
        
        // Pass through environment variables
        let grpc_url = std::env::var("MAOWBOT_GRPC_URL").unwrap_or_else(|_| "https://127.0.0.1:9999".to_string());
        let grpc_pass = std::env::var("MAOWBOT_GRPC_PASSPHRASE").unwrap_or_default();
        let grpc_ca = std::env::var("MAOWBOT_GRPC_CA").unwrap_or_else(|_| "certs/server.crt".to_string());

        cmd.env("MAOWBOT_GRPC_URL", grpc_url)
            .env("MAOWBOT_GRPC_PASSPHRASE", grpc_pass)
            .env("MAOWBOT_GRPC_CA", grpc_ca)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        if let Ok(log_level) = std::env::var("RUST_LOG") {
            cmd.env("RUST_LOG", log_level);
        }

        let mut child = cmd.spawn()?;
        let pid = child.id();
        info!("Overlay process started with PID: {:?}", pid);

        // Capture stdout
        if let Some(stdout) = child.stdout.take() {
            tokio::spawn(async move {
                let reader = BufReader::new(stdout);
                let mut lines = reader.lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    info!("[overlay] {}", line);
                }
            });
        }

        // Capture stderr
        if let Some(stderr) = child.stderr.take() {
            tokio::spawn(async move {
                let reader = BufReader::new(stderr);
                let mut lines = reader.lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    error!("[overlay] {}", line);
                }
            });
        }

        *guard = Some(child);
        drop(guard); // Release the lock before spawning monitor

        // Send event if we have an event sender
        if let Some(tx) = &self.event_tx {
            let _ = tx.send(AppEvent::OverlayStatusChanged(true));
        }

        // Monitor the overlay process
        let process_handle = self.overlay_process.clone();
        let event_tx_clone = self.event_tx.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

                let mut proc_guard = process_handle.lock().await;
                if let Some(child) = proc_guard.as_mut() {
                    match child.try_wait() {
                        Ok(Some(status)) => {
                            // Process has exited
                            info!("Overlay process exited with status: {:?}", status);
                            if !status.success() {
                                error!("Overlay exited with error code: {:?}", status.code());
                            }
                            *proc_guard = None;
                            if let Some(tx) = &event_tx_clone {
                                let _ = tx.send(AppEvent::OverlayStatusChanged(false));
                            }
                            break;
                        }
                        Ok(None) => {
                            // Still running
                            continue;
                        }
                        Err(e) => {
                            error!("Error checking overlay status: {}", e);
                            *proc_guard = None;
                            if let Some(tx) = &event_tx_clone {
                                let _ = tx.send(AppEvent::OverlayStatusChanged(false));
                            }
                            break;
                        }
                    }
                } else {
                    // No process to monitor
                    break;
                }
            }
        });
        
        Ok(())
    }

    /// Stop a process
    pub async fn stop(&self, process_type: ProcessType) -> Result<(), Box<dyn std::error::Error>> {
        let process = match process_type {
            ProcessType::Server => &self.server_process,
            ProcessType::Overlay => &self.overlay_process,
        };

        let mut guard = process.lock().await;
        
        if let Some(mut child) = guard.take() {
            info!("Stopping {:?} process", process_type);
            
            // Try graceful shutdown first on Windows
            #[cfg(windows)]
            {
                use windows::Win32::System::Threading::{OpenProcess, TerminateProcess, PROCESS_TERMINATE};
                use windows::Win32::Foundation::CloseHandle;
                
                if let Some(pid) = child.id() {
                    unsafe {
                        match OpenProcess(PROCESS_TERMINATE, false, pid) {
                            Ok(handle) => {
                                let _ = TerminateProcess(handle, 0);
                                let _ = CloseHandle(handle);
                            }
                            Err(e) => {
                                warn!("Failed to open process for termination: {}", e);
                            }
                        }
                    }
                }
            }
            
            // Then use kill as fallback
            match child.kill().await {
                Ok(_) => {
                    info!("{:?} process stopped", process_type);
                    
                    // Send event if we have an event sender
                    if let ProcessType::Overlay = process_type {
                        if let Some(tx) = &self.event_tx {
                            let _ = tx.send(AppEvent::OverlayStatusChanged(false));
                        }
                    }
                }
                Err(e) => {
                    error!("Failed to stop {:?}: {}", process_type, e);
                    return Err(e.into());
                }
            }
        } else {
            info!("No {:?} process to stop", process_type);
        }
        
        Ok(())
    }

    /// Stop all managed processes
    pub async fn stop_all(&self) -> Result<(), Box<dyn std::error::Error>> {
        // Stop overlay first, then server
        if let Err(e) = self.stop(ProcessType::Overlay).await {
            error!("Failed to stop overlay: {}", e);
        }
        
        if let Err(e) = self.stop(ProcessType::Server).await {
            error!("Failed to stop server: {}", e);
        }
        
        Ok(())
    }

    /// Ensure server is running and return connection info
    pub async fn ensure_server_running(&self) -> Result<String, Box<dyn std::error::Error>> {
        if !self.is_running(ProcessType::Server).await {
            self.start_server().await?;
        }
        
        // Return the server URL
        Ok("https://127.0.0.1:9999".to_string())
    }
}

impl Default for ProcessManager {
    fn default() -> Self {
        Self::new()
    }
}