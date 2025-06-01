#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod egui_renderer;
mod layout_constants;
mod process_manager;
mod settings;

use anyhow::Result;
use crossbeam_channel::{bounded, Sender, Receiver};
use eframe::egui;
use maowbot_ui::{AppState, AppEvent, SharedGrpcClient};
use maowbot_ui::events::ChatCommand;
use process_manager::ProcessManager;
use std::sync::{Arc, Mutex};
use tracing_subscriber::EnvFilter;

#[derive(Clone)]
pub enum WindowMode {
    Main,
    Secondary,
}

struct DesktopApp {
    state: AppState,
    renderer: egui_renderer::EguiRenderer,
    process_manager: Arc<Mutex<ProcessManager>>,
    event_rx: Receiver<AppEvent>,
    event_tx: Sender<AppEvent>,
    command_tx: Sender<ChatCommand>,
    window_mode: WindowMode,
    secondary_window_id: Option<egui::ViewportId>,
    should_open_secondary: bool,
    secondary_renderer: Option<egui_renderer::EguiRenderer>,
}

impl DesktopApp {
    fn new(cc: &eframe::CreationContext<'_>, window_mode: WindowMode, state: Option<AppState>) -> Result<Self> {
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

        // Use provided state or create new
        let state = state.unwrap_or_else(AppState::new);

        // Only start gRPC client for main window
        if matches!(window_mode, WindowMode::Main) {
            SharedGrpcClient::start(
                "maowbot-gui".to_string(),
                event_tx.clone(),
                command_rx,
            );
        }

        // Create process manager
        let process_manager = Arc::new(Mutex::new(ProcessManager::new(event_tx.clone())));

        Ok(Self {
            state,
            renderer: egui_renderer::EguiRenderer::new(window_mode.clone()),
            process_manager,
            event_rx,
            event_tx,
            command_tx,
            window_mode,
            secondary_window_id: None,
            should_open_secondary: false,
            secondary_renderer: None,
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

        // Handle deferred secondary window opening
        if self.should_open_secondary {
            self.should_open_secondary = false;
            let settings = self.renderer.get_settings();
            self.secondary_renderer = Some(egui_renderer::EguiRenderer::new_with_settings(WindowMode::Secondary, settings));
        }
        
        // Show secondary window if undocked
        if !*self.state.is_docked.lock().unwrap() {
            let viewport_id = egui::ViewportId::from_hash_of("secondary_window");
            self.secondary_window_id = Some(viewport_id);
            
            if let Some(renderer) = &mut self.secondary_renderer {
                let state_clone = self.state.clone();
                let main_ctx = ctx.clone();
                let settings = self.renderer.get_settings();
                ctx.show_viewport_deferred(
                    viewport_id,
                    egui::ViewportBuilder::default()
                        .with_title("maowbot - Secondary View")
                        .with_inner_size([800.0, 600.0])
                        .with_min_inner_size([600.0, 400.0]),
                    move |ctx, _class| {
                        // Create a renderer with shared settings
                        let mut temp_renderer = egui_renderer::EguiRenderer::new_with_settings(WindowMode::Secondary, settings.clone());
                        temp_renderer.render_secondary_window(ctx, &state_clone);
                        
                        // Check if we just docked and need to notify main window
                        if *state_clone.is_docked.lock().unwrap() {
                            main_ctx.request_repaint();
                        }
                        
                        ctx.request_repaint();
                    },
                );
            }
        } else {
            // Clean up when docked
            self.secondary_renderer = None;
            if let Some(id) = self.secondary_window_id.take() {
                ctx.send_viewport_cmd_to(id, egui::ViewportCommand::Close);
            }
        }
        

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
                maowbot_ui::UIEvent::Undock => {
                    if matches!(self.window_mode, WindowMode::Main) {
                        *self.state.is_docked.lock().unwrap() = false;
                        self.should_open_secondary = true;
                    }
                }
                maowbot_ui::UIEvent::Dock => {
                    *self.state.is_docked.lock().unwrap() = true;
                    // Window will be closed in the next frame
                }
                _ => {}
            }
        }

        // Request repaint for animations and state changes
        ctx.request_repaint();
        
        // If undocked, request more frequent repaints to catch dock events quickly
        if !*self.state.is_docked.lock().unwrap() {
            ctx.request_repaint_after(std::time::Duration::from_millis(50));
        }
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
            .with_inner_size([1400.0, 900.0])
            .with_min_inner_size([1000.0, 700.0]),
        ..Default::default()
    };

    let result = eframe::run_native(
        "maowbot Control Center",
        native_options,
        Box::new(|cc| Ok(Box::new(DesktopApp::new(cc, WindowMode::Main, None).unwrap()))),
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