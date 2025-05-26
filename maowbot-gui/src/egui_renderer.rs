use crossbeam_channel::Sender;
use egui::{Color32, RichText, ScrollArea, TextEdit};
use maowbot_ui::{AppState, UIEvent};
use maowbot_ui::events::ChatCommand;  // Correct import path
use std::sync::{Arc, Mutex};

use crate::process_manager::ProcessManager;

pub struct EguiRenderer {
    input_buffer: String,
    show_settings: bool,
}

impl EguiRenderer {
    pub fn new() -> Self {
        Self {
            input_buffer: String::new(),
            show_settings: false,
        }
    }

    pub fn handle_ui_event(
        &mut self,
        ctx: &egui::Context,
        state: &AppState,
        command_tx: &Sender<ChatCommand>,
        process_manager: &Arc<Mutex<ProcessManager>>,
    ) -> Option<UIEvent> {
        let mut result = None;

        // Top panel with controls
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            ui.add_space(5.0);
            ui.horizontal(|ui| {
                ui.heading("maowbot Control Center");

                ui.separator();

                // Status indicators
                let grpc_connected = *state.grpc_connected.lock().unwrap();
                let overlay_running = *state.overlay_running.lock().unwrap();

                ui.label("gRPC:");
                if grpc_connected {
                    ui.colored_label(Color32::from_rgb(0, 255, 0), "●");
                } else {
                    ui.colored_label(Color32::from_rgb(255, 0, 0), "●");
                }

                ui.separator();

                ui.label("Overlay:");
                if overlay_running {
                    ui.colored_label(Color32::from_rgb(0, 255, 0), "●");
                } else {
                    ui.colored_label(Color32::from_rgb(255, 0, 0), "●");
                }

                ui.separator();

                // Control buttons
                if overlay_running {
                    if ui.button("Stop Overlay").clicked() {
                        let pm = process_manager.lock().unwrap().clone();
                        tokio::spawn(async move {
                            if let Err(e) = pm.stop_overlay().await {
                                tracing::error!("Failed to stop overlay: {}", e);
                            }
                        });
                    }
                    if ui.button("Restart Overlay").clicked() {
                        let pm = process_manager.lock().unwrap().clone();
                        tokio::spawn(async move {
                            if let Err(e) = pm.restart_overlay().await {
                                tracing::error!("Failed to restart overlay: {}", e);
                            }
                        });
                    }
                } else {
                    if ui.button("Start Overlay").clicked() {
                        let pm = process_manager.lock().unwrap().clone();
                        tokio::spawn(async move {
                            if let Err(e) = pm.start_overlay().await {
                                tracing::error!("Failed to start overlay: {}", e);
                            }
                        });
                    }
                }

                ui.separator();

                if ui.button("⚙ Settings").clicked() {
                    self.show_settings = !self.show_settings;
                }

                // Right-aligned quit button
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Quit").clicked() {
                        result = Some(UIEvent::Quit);
                    }
                });
            });
            ui.add_space(5.0);
        });

        // Settings window
        if self.show_settings {
            egui::Window::new("Settings")
                .collapsible(false)
                .show(ctx, |ui| {
                    ui.heading("Connection Settings");
                    ui.separator();

                    ui.horizontal(|ui| {
                        ui.label("gRPC URL:");
                        ui.label(std::env::var("MAOWBOT_GRPC_URL").unwrap_or_default());
                    });

                    ui.horizontal(|ui| {
                        ui.label("CA Path:");
                        ui.label(std::env::var("MAOWBOT_GRPC_CA").unwrap_or_default());
                    });

                    ui.separator();

                    if ui.button("Close").clicked() {
                        self.show_settings = false;
                    }
                });
        }

        // Main chat panel
        egui::CentralPanel::default().show(ctx, |ui| {
            let available_height = ui.available_height();

            // Chat area
            let chat_height = available_height - 50.0;
            ScrollArea::vertical()
                .max_height(chat_height)
                .auto_shrink([false; 2])
                .stick_to_bottom(true)
                .show(ui, |ui| {
                    let chat_state = state.chat_state.lock().unwrap();

                    for msg in chat_state.messages() {
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(format!("{}:", msg.author))
                                    .color(Color32::from_rgb(255, 200, 50))
                                    .strong(),
                            );
                            ui.label(&msg.text);
                        });
                        ui.add_space(2.0);
                    }
                });

            ui.separator();

            // Input area
            ui.horizontal(|ui| {
                let response = ui.add(
                    TextEdit::singleline(&mut self.input_buffer)
                        .desired_width(ui.available_width() - 80.0)
                        .hint_text("Type a message...")
                );

                if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    if !self.input_buffer.is_empty() {
                        let _ = command_tx.send(ChatCommand::SendMessage(
                            self.input_buffer.clone()
                        ));
                        self.input_buffer.clear();
                        response.request_focus();
                    }
                }

                if ui.button("Send").clicked() && !self.input_buffer.is_empty() {
                    let _ = command_tx.send(ChatCommand::SendMessage(
                        self.input_buffer.clone()
                    ));
                    self.input_buffer.clear();
                }
            });
        });

        result
    }
}