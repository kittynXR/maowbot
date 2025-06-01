use anyhow::Result;
use crossbeam_channel::Sender;
use maowbot_common_ui::AppEvent;
use std::process::Stdio;
use tokio::process::{Child, Command};
use std::sync::Arc;
use tokio::sync::Mutex as TokioMutex;
use std::path::PathBuf;
use tokio::io::{AsyncBufReadExt, BufReader};

#[derive(Clone)]
pub struct ProcessManager {
    overlay_process: Arc<TokioMutex<Option<Child>>>,
    event_tx: Sender<AppEvent>,
}

impl ProcessManager {
    pub fn new(event_tx: Sender<AppEvent>) -> Self {
        Self {
            overlay_process: Arc::new(TokioMutex::new(None)),
            event_tx,
        }
    }

    pub fn start_overlay(&self) -> impl std::future::Future<Output = Result<()>> + Send + 'static {
        let overlay_process = self.overlay_process.clone();
        let event_tx = self.event_tx.clone();

        async move {
            let mut proc_guard = overlay_process.lock().await;

            if proc_guard.is_some() {
                tracing::info!("Overlay already running");
                return Ok(());
            }

            // Try to find the overlay executable
            let exe_name = if cfg!(windows) { "maowbot-overlay.exe" } else { "maowbot-overlay" };

            // Try multiple paths
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
                    tracing::debug!("Checking path: {:?} - exists: {}", p, exists);
                    exists
                })
                .ok_or_else(|| anyhow::anyhow!("Could not find maowbot-overlay executable"))?;

            tracing::info!("Starting overlay from: {:?}", overlay_path);

            let mut cmd = Command::new(&overlay_path);

            // Pass through environment variables
            let grpc_url = std::env::var("MAOWBOT_GRPC_URL").unwrap_or_else(|_| "https://localhost:9999".to_string());
            let grpc_pass = std::env::var("MAOWBOT_GRPC_PASSPHRASE").unwrap_or_default();
            let grpc_ca = std::env::var("MAOWBOT_GRPC_CA").unwrap_or_else(|_| "certs/server.crt".to_string());

            tracing::info!("Starting overlay with GRPC_URL: {}", grpc_url);

            cmd.env("MAOWBOT_GRPC_URL", grpc_url)
                .env("MAOWBOT_GRPC_PASSPHRASE", grpc_pass)
                .env("MAOWBOT_GRPC_CA", grpc_ca)
                .env("RUST_LOG", "maowbot_overlay=debug,maowbot_ui=debug")
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());

            match cmd.spawn() {
                Ok(mut child) => {
                    let pid = child.id();
                    tracing::info!("Overlay process started with PID: {:?}", pid);

                    // Capture stdout
                    if let Some(stdout) = child.stdout.take() {
                        let reader = BufReader::new(stdout);
                        tokio::spawn(async move {
                            let mut lines = reader.lines();
                            while let Ok(Some(line)) = lines.next_line().await {
                                tracing::info!("[overlay stdout] {}", line);
                            }
                        });
                    }

                    // Capture stderr
                    if let Some(stderr) = child.stderr.take() {
                        let reader = BufReader::new(stderr);
                        tokio::spawn(async move {
                            let mut lines = reader.lines();
                            while let Ok(Some(line)) = lines.next_line().await {
                                tracing::error!("[overlay stderr] {}", line);
                            }
                        });
                    }

                    *proc_guard = Some(child);
                    drop(proc_guard); // Release the lock before spawning monitor

                    let _ = event_tx.send(AppEvent::OverlayStatusChanged(true));

                    // Monitor the process
                    let process_handle = overlay_process.clone();
                    let event_tx_clone = event_tx.clone();
                    tokio::spawn(async move {
                        loop {
                            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

                            let mut proc_guard = process_handle.lock().await;
                            if let Some(child) = proc_guard.as_mut() {
                                match child.try_wait() {
                                    Ok(Some(status)) => {
                                        // Process has exited
                                        tracing::info!("Overlay process exited with status: {:?}", status);
                                        if !status.success() {
                                            tracing::error!("Overlay exited with error code: {:?}", status.code());
                                        }
                                        *proc_guard = None;
                                        let _ = event_tx_clone.send(AppEvent::OverlayStatusChanged(false));
                                        break;
                                    }
                                    Ok(None) => {
                                        // Still running
                                        continue;
                                    }
                                    Err(e) => {
                                        tracing::error!("Error checking overlay status: {}", e);
                                        *proc_guard = None;
                                        let _ = event_tx_clone.send(AppEvent::OverlayStatusChanged(false));
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
                Err(e) => {
                    tracing::error!("Failed to start overlay: {}", e);
                    Err(e.into())
                }
            }
        }
    }

    // stop_overlay and restart_overlay remain the same...
    pub fn stop_overlay(&self) -> impl std::future::Future<Output = Result<()>> + Send + 'static {
        let overlay_process = self.overlay_process.clone();
        let event_tx = self.event_tx.clone();

        async move {
            let mut proc_guard = overlay_process.lock().await;

            if let Some(mut child) = proc_guard.take() {
                tracing::info!("Stopping overlay process");

                // Try graceful shutdown first (on Windows, this sends WM_CLOSE)
                #[cfg(windows)]
                {
                    use windows::Win32::System::Threading::{OpenProcess, TerminateProcess};
                    use windows::Win32::Foundation::CloseHandle;
                    use windows::Win32::System::Threading::PROCESS_TERMINATE;

                    if let Some(pid) = child.id() {
                        unsafe {
                            match OpenProcess(PROCESS_TERMINATE, false, pid) {
                                Ok(handle) => {
                                    let _ = TerminateProcess(handle, 0);
                                    let _ = CloseHandle(handle);
                                }
                                Err(e) => {
                                    tracing::warn!("Failed to open process for termination: {}", e);
                                }
                            }
                        }
                    }
                }

                // Then use kill as fallback
                match child.kill().await {
                    Ok(_) => {
                        tracing::info!("Overlay process stopped");
                        let _ = event_tx.send(AppEvent::OverlayStatusChanged(false));
                    }
                    Err(e) => {
                        tracing::error!("Failed to stop overlay: {}", e);
                        return Err(e.into());
                    }
                }
            } else {
                tracing::info!("No overlay process to stop");
            }

            Ok(())
        }
    }

    pub fn restart_overlay(&self) -> impl std::future::Future<Output = Result<()>> + Send + 'static {
        let stop_fut = self.stop_overlay();
        let start_fut = self.start_overlay();

        async move {
            tracing::info!("Restarting overlay...");
            stop_fut.await?;
            tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
            start_fut.await
        }
    }

    pub fn force_stop_overlay(&self) -> impl std::future::Future<Output = Result<()>> + Send + 'static {
        let overlay_process = self.overlay_process.clone();
        let event_tx = self.event_tx.clone();

        async move {
            let mut proc_guard = overlay_process.lock().await;

            if let Some(mut child) = proc_guard.take() {
                tracing::info!("Force stopping overlay process");

                // On Windows, use TerminateProcess immediately
                #[cfg(windows)]
                {
                    use windows::Win32::System::Threading::{OpenProcess, TerminateProcess};
                    use windows::Win32::Foundation::CloseHandle;
                    use windows::Win32::System::Threading::PROCESS_TERMINATE;

                    if let Some(pid) = child.id() {
                        unsafe {
                            match OpenProcess(PROCESS_TERMINATE, false, pid) {
                                Ok(handle) => {
                                    let _ = TerminateProcess(handle, 1);
                                    let _ = CloseHandle(handle);
                                    tracing::info!("Terminated overlay process");
                                }
                                Err(e) => {
                                    tracing::warn!("Failed to terminate process: {}", e);
                                }
                            }
                        }
                    }
                }

                // Also try kill
                let _ = child.kill().await;
                let _ = event_tx.send(AppEvent::OverlayStatusChanged(false));
            }

            Ok(())
        }
    }
}