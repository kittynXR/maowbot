#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod egui_renderer;
mod process_manager;

use anyhow::Result;
use crossbeam_channel::{bounded, Sender, Receiver};
use eframe::egui;
use maowbot_ui::{AppState, AppEvent, SharedGrpcClient};
use maowbot_ui::events::ChatCommand;
use process_manager::ProcessManager;
use std::sync::{Arc, Mutex};
use tracing_subscriber::EnvFilter;

struct DesktopApp {
    state: AppState,
    renderer: egui_renderer::EguiRenderer,
    process_manager: Arc<Mutex<ProcessManager>>,
    event_rx: Receiver<AppEvent>,
    event_tx: Sender<AppEvent>,
    command_tx: Sender<ChatCommand>,
}

impl DesktopApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Result<Self> {
        // Configure egui style
        let mut style = (*cc.egui_ctx.style()).clone();
        style.visuals.window_shadow = egui::epaint::Shadow {
            offset: [0, 0],
            blur: 0,
            spread: 0,
            color: egui::Color32::TRANSPARENT,
        };
        cc.egui_ctx.set_style(style);

        // Create channels
        let (event_tx, event_rx) = bounded(100);
        let (command_tx, command_rx) = bounded(100);

        // Create shared state
        let state = AppState::new();

        // Start gRPC client
        SharedGrpcClient::start(
            "maowbot-gui".to_string(),
            event_tx.clone(),
            command_rx,
        );

        // Create process manager
        let process_manager = Arc::new(Mutex::new(ProcessManager::new(event_tx.clone())));

        Ok(Self {
            state,
            renderer: egui_renderer::EguiRenderer::new(),
            process_manager,
            event_rx,
            event_tx,
            command_tx,
        })
    }

    fn handle_events(&mut self) {
        while let Ok(event) = self.event_rx.try_recv() {
            match event {
                AppEvent::Chat(chat_event) => {
                    let mut chat_state = self.state.chat_state.lock().unwrap();
                    chat_state.add_message(chat_event);
                }
                AppEvent::OverlayStatusChanged(running) => {
                    *self.state.overlay_running.lock().unwrap() = running;
                }
                AppEvent::GrpcStatusChanged(connected) => {
                    *self.state.grpc_connected.lock().unwrap() = connected;
                }
                AppEvent::Shutdown => {
                    // Don't exit immediately, let the app handle it
                }
            }
        }
    }

    fn cleanup(&self) {
        tracing::info!("Cleaning up before exit...");

        // Stop the overlay if it's running
        let overlay_running = *self.state.overlay_running.lock().unwrap();
        if overlay_running {
            let pm = self.process_manager.lock().unwrap().clone();

            // Use tokio::task::block_in_place to run async code in sync context
            tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(async move {
                    match tokio::time::timeout(
                        tokio::time::Duration::from_secs(5),
                        pm.stop_overlay()
                    ).await {
                        Ok(result) => {
                            if let Err(e) = result {
                                tracing::error!("Error stopping overlay: {}", e);
                            } else {
                                tracing::info!("Overlay stopped successfully");
                            }
                        }
                        Err(_) => {
                            tracing::error!("Timeout stopping overlay");
                        }
                    }
                });
            });
        }

        // Send shutdown event
        let _ = self.event_tx.send(AppEvent::Shutdown);

        // Give a moment for things to clean up
        std::thread::sleep(std::time::Duration::from_millis(500));
    }
}

impl eframe::App for DesktopApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Handle events
        self.handle_events();

        // Process UI events
        if let Some(event) = self.renderer.handle_ui_event(
            ctx,
            &self.state,
            &self.command_tx,
            &self.process_manager,
        ) {
            match event {
                maowbot_ui::UIEvent::Quit => {
                    self.cleanup();
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
                _ => {}
            }
        }

        // Request repaint for animations
        ctx.request_repaint();
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        // This is called when the window is closing
        self.cleanup();
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::from_default_env()
                .add_directive("maowbot_gui=info".parse().unwrap()),
        )
        .init();

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("maowbot Control Center")
            .with_inner_size([1200.0, 800.0])
            .with_min_inner_size([800.0, 600.0]),
        ..Default::default()
    };

    let result = eframe::run_native(
        "maowbot Control Center",
        native_options,
        Box::new(|cc| Ok(Box::new(DesktopApp::new(cc).unwrap()))),
    );

    match result {
        Ok(_) => {
            tracing::info!("Application exited normally");
            Ok(())
        }
        Err(e) => {
            tracing::error!("Application error: {}", e);
            Err(anyhow::anyhow!("eframe error: {}", e))
        }
    }
}