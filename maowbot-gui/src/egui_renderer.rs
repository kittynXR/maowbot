use crossbeam_channel::Sender;
use egui::{Color32, RichText, ScrollArea, TextEdit, Vec2, Rect};
use maowbot_ui::{AppState, UIEvent, LayoutSection};
use maowbot_ui::events::ChatCommand;
use std::sync::{Arc, Mutex};

use crate::process_manager::ProcessManager;
use crate::WindowMode;

pub struct EguiRenderer {
    input_buffer: String,
    secondary_input_buffer: String,
    show_settings: bool,
    window_mode: WindowMode,
}

impl EguiRenderer {
    pub fn new(window_mode: WindowMode) -> Self {
        Self {
            input_buffer: String::new(),
            secondary_input_buffer: String::new(),
            show_settings: false,
            window_mode,
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
        let is_docked = *state.is_docked.lock().unwrap();

        match self.window_mode {
            WindowMode::Main => {
                if is_docked {
                    // Show full UI when docked
                    result = self.render_full_ui(ctx, state, command_tx, process_manager);
                } else {
                    // Show only main chat and right panel when undocked
                    result = self.render_main_window_undocked(ctx, state, command_tx, process_manager);
                }
            }
            WindowMode::Secondary => {
                // This shouldn't be called for secondary window
            }
        }

        result
    }

    pub fn render_secondary_window(&mut self, ctx: &egui::Context, state: &AppState) {
        // Check if we should close this window
        if *state.is_docked.lock().unwrap() {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }

        let mut should_dock = false;

        // Top panel with dock button
        egui::TopBottomPanel::top("secondary_top_panel").show(ctx, |ui| {
            ui.add_space(5.0);
            ui.horizontal(|ui| {
                ui.heading("maowbot - Secondary View");
                
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Dock").clicked() {
                        should_dock = true;
                    }
                });
            });
            ui.add_space(5.0);
        });

        // Main content - secondary chat and tabs
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal_top(|ui| {
                ui.set_height(ui.available_height());
                
                let width = ui.available_width();
                let chat_width = width * 0.4;
                let tabs_width = width * 0.6 - 5.0;

                // Secondary chat
                ui.allocate_ui_with_layout(
                    egui::vec2(chat_width, ui.available_height()),
                    egui::Layout::top_down(egui::Align::LEFT),
                    |ui| {
                        ui.set_width(chat_width);
                        let (dummy_tx, _) = crossbeam_channel::unbounded();
                        self.render_left_chat(ui, state, &dummy_tx);
                    },
                );

                ui.add(egui::Separator::default().vertical());

                // Tabs area
                ui.allocate_ui_with_layout(
                    egui::vec2(tabs_width, ui.available_height()),
                    egui::Layout::top_down(egui::Align::LEFT),
                    |ui| {
                        ui.set_width(tabs_width);
                        self.render_tab_area(ui, state);
                    },
                );
            });
        });

        if should_dock {
            *state.is_docked.lock().unwrap() = true;
        }
    }

    fn render_full_ui(
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
                ui.heading("maowbot");

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

                // Right-aligned buttons
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Quit").clicked() {
                        result = Some(UIEvent::Quit);
                    }
                    
                    ui.separator();
                    
                    if ui.button("Undock").clicked() {
                        result = Some(UIEvent::Undock);
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

        // Main content area with 4 sections
        egui::CentralPanel::default().show(ctx, |ui| {
            // Calculate section widths with proper margins
            let total_width = ui.available_width();
            let margin = 5.0;
            let separator_count = 3;
            let separator_total_width = separator_count as f32 * 2.0;
            let usable_width = total_width - separator_total_width - (margin * 2.0);
            
            // Calculate right panel width based on video needs
            let available_height = ui.available_height() - margin * 2.0;
            let video_area_height = available_height * 0.6 - 10.0;
            let aspect_ratio = 16.0 / 9.0;
            let ideal_video_width = video_area_height * aspect_ratio + 20.0;
            
            // Right panel constraints
            let min_right_width = 150.0;
            let max_right_width = usable_width * 0.25; // Max 25% of total
            let right_panel_width = ideal_video_width.clamp(min_right_width, max_right_width);
            
            // Chat constraints
            let max_main_chat_width = 400.0;
            let ideal_main_chat_width = 340.0;
            
            // Distribute remaining width
            let remaining_width = usable_width - right_panel_width;
            
            // Calculate main chat width with constraints
            let proposed_main_chat = remaining_width * 0.40;
            let main_chat_width = proposed_main_chat.min(max_main_chat_width);
            
            // Redistribute extra space to tabs if main chat is at max
            let extra_space = if proposed_main_chat > max_main_chat_width {
                proposed_main_chat - max_main_chat_width
            } else {
                0.0
            };
            
            let left_chat_width = remaining_width * 0.25;
            let tab_area_width = remaining_width * 0.35 + extra_space;
            
            let section_widths = [
                left_chat_width,         // Left chat
                tab_area_width,          // Tab area (gets extra space)
                main_chat_width,         // Main chat (capped)
                right_panel_width,       // Right panel
            ];

            ui.add_space(margin);
            ui.horizontal_top(|ui| {
                ui.set_height(ui.available_height() - margin * 2.0);
                
                let layout_order = state.layout_order.lock().unwrap().clone();
                
                for (i, section) in layout_order.iter().enumerate() {
                    if i > 0 {
                        ui.add(egui::Separator::default().vertical());
                    }

                    let width = match section {
                        LayoutSection::LeftChat => section_widths[0],
                        LayoutSection::TabArea => section_widths[1],
                        LayoutSection::MainChat => section_widths[2],
                        LayoutSection::RightPanel => section_widths[3],
                    };

                    ui.allocate_ui_with_layout(
                        egui::vec2(width, ui.available_height()),
                        egui::Layout::top_down(egui::Align::LEFT),
                        |ui| {
                            ui.set_width(width);
                            match section {
                                LayoutSection::LeftChat => self.render_left_chat(ui, state, command_tx),
                                LayoutSection::TabArea => self.render_tab_area(ui, state),
                                LayoutSection::MainChat => self.render_main_chat(ui, state, command_tx),
                                LayoutSection::RightPanel => self.render_right_panel(ui),
                            }
                        },
                    );
                }
            });
        });

        result
    }

    fn render_main_window_undocked(
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
                ui.heading("maowbot - Main View");

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

                // Right-aligned buttons
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Quit").clicked() {
                        result = Some(UIEvent::Quit);
                    }
                    
                    ui.separator();
                    
                    if ui.button("Dock").clicked() {
                        result = Some(UIEvent::Dock);
                    }
                });
            });
            ui.add_space(5.0);
        });

        // Main content - only main chat and right panel
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal_top(|ui| {
                ui.set_height(ui.available_height());
                
                let total_width = ui.available_width();
                let separator_width = 5.0;
                
                // Chat constraints (like standard Twitch chat)
                let min_chat_width = 280.0;  // Minimum readable width
                let ideal_chat_width = 340.0; // Standard Twitch chat width
                let max_chat_width = 400.0;   // Maximum before it gets too wide
                
                // Calculate right panel width based on video player needs
                let available_height = ui.available_height();
                let video_area_height = available_height * 0.6 - 10.0;
                let aspect_ratio = 16.0 / 9.0;
                
                // Start with chat at ideal width
                let chat_width = if total_width - separator_width > ideal_chat_width + 200.0 {
                    // We have enough space, use ideal chat width
                    ideal_chat_width
                } else {
                    // Limited space, use minimum
                    min_chat_width
                };
                
                // Video panel gets the rest, ensuring it can show video properly
                let right_width = total_width - chat_width - separator_width;
                
                // If we have extra space and chat is at ideal width, expand chat up to max
                let final_chat_width = if right_width > video_area_height * aspect_ratio + 50.0 {
                    // Video has enough space, can expand chat
                    let extra = right_width - (video_area_height * aspect_ratio + 50.0);
                    (chat_width + extra).min(max_chat_width)
                } else {
                    chat_width
                };
                
                let final_right_width = total_width - final_chat_width - separator_width;

                // Main chat
                ui.allocate_ui_with_layout(
                    egui::vec2(final_chat_width, ui.available_height()),
                    egui::Layout::top_down(egui::Align::LEFT),
                    |ui| {
                        ui.set_width(final_chat_width);
                        self.render_main_chat(ui, state, command_tx);
                    },
                );

                ui.add(egui::Separator::default().vertical());

                // Right panel
                ui.allocate_ui_with_layout(
                    egui::vec2(final_right_width, ui.available_height()),
                    egui::Layout::top_down(egui::Align::LEFT),
                    |ui| {
                        ui.set_width(final_right_width);
                        self.render_right_panel(ui);
                    },
                );
            });
        });

        result
    }

    fn render_left_chat(&mut self, ui: &mut egui::Ui, state: &AppState, _command_tx: &Sender<ChatCommand>) {
        let available_height = ui.available_height();
        
        // Secondary chat area
        ui.vertical(|ui| {
            ui.set_height(available_height);
            
            ui.label(RichText::new("Secondary Chat").strong());
            ui.separator();
            
            let chat_height = available_height - 80.0;
            ScrollArea::vertical()
                .id_source("secondary_chat_scroll")
                .max_height(chat_height)
                .auto_shrink([false; 2])
                .stick_to_bottom(true)
                .show(ui, |ui| {
                    let chat_state = state.secondary_chat_state.lock().unwrap();
                    
                    for msg in chat_state.messages() {
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(format!("{}:", msg.author))
                                    .color(Color32::from_rgb(200, 150, 255))
                                    .strong(),
                            );
                            ui.label(&msg.text);
                        });
                        ui.add_space(2.0);
                    }
                });
            
            ui.separator();
            
            // Secondary input area
            ui.horizontal(|ui| {
                let response = ui.add(
                    TextEdit::singleline(&mut self.secondary_input_buffer)
                        .desired_width(ui.available_width() - 50.0)
                        .hint_text("Type a message...")
                );
                
                if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    if !self.secondary_input_buffer.is_empty() {
                        // TODO: Send to secondary chat
                        self.secondary_input_buffer.clear();
                        response.request_focus();
                    }
                }
                
                if ui.button("Send").clicked() && !self.secondary_input_buffer.is_empty() {
                    // TODO: Send to secondary chat
                    self.secondary_input_buffer.clear();
                }
            });
        });
    }

    fn render_tab_area(&mut self, ui: &mut egui::Ui, state: &AppState) {
        let available_height = ui.available_height();
        
        ui.vertical(|ui| {
            ui.set_height(available_height);
            
            // Tab buttons
            ui.horizontal(|ui| {
                let mut active_tab = state.active_tab.lock().unwrap();
                
                if ui.selectable_label(*active_tab == "Multiview", "Multiview").clicked() {
                    *active_tab = "Multiview".to_string();
                }
                ui.separator();
                
                if ui.selectable_label(*active_tab == "Analytics", "Analytics").clicked() {
                    *active_tab = "Analytics".to_string();
                }
                ui.separator();
                
                if ui.selectable_label(*active_tab == "Moderation", "Moderation").clicked() {
                    *active_tab = "Moderation".to_string();
                }
                ui.separator();
                
                if ui.selectable_label(*active_tab == "Discord", "Discord").clicked() {
                    *active_tab = "Discord".to_string();
                }
                ui.separator();
                
                if ui.selectable_label(*active_tab == "Browser", "Browser").clicked() {
                    *active_tab = "Browser".to_string();
                }
            });
            
            ui.separator();
            
            // Tab content
            let content_height = available_height - 40.0;
            ui.allocate_ui_with_layout(
                egui::vec2(ui.available_width(), content_height),
                egui::Layout::top_down(egui::Align::LEFT),
                |ui| {
                    let active_tab = state.active_tab.lock().unwrap().clone();
                    match active_tab.as_str() {
                        "Multiview" => {
                            self.render_video_grid(ui, 2, 2);
                        }
                        "Analytics" => {
                            ui.centered_and_justified(|ui| {
                                ui.label("Analytics Dashboard\n(Coming Soon)");
                            });
                        }
                        "Moderation" => {
                            ui.centered_and_justified(|ui| {
                                ui.label("Moderation Tools\n(Coming Soon)");
                            });
                        }
                        "Discord" => {
                            ui.centered_and_justified(|ui| {
                                ui.label("Discord Integration\n(CEF Embed Placeholder)");
                            });
                        }
                        "Browser" => {
                            ui.centered_and_justified(|ui| {
                                ui.label("Web Browser\n(CEF Embed Placeholder)");
                            });
                        }
                        _ => {}
                    }
                },
            );
        });
    }

    fn render_main_chat(&mut self, ui: &mut egui::Ui, state: &AppState, command_tx: &Sender<ChatCommand>) {
        let available_height = ui.available_height();
        
        ui.vertical(|ui| {
            ui.set_height(available_height);
            
            ui.label(RichText::new("Main Stream Chat").strong());
            ui.separator();
            
            // Chat area
            let chat_height = available_height - 80.0;
            ScrollArea::vertical()
                .id_source("main_chat_scroll")
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
                        .desired_width(ui.available_width() - 60.0)
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
    }

    fn render_right_panel(&mut self, ui: &mut egui::Ui) {
        let available_size = ui.available_size();
        let available_width = available_size.x;
        let available_height = available_size.y;
        
        let button_area_height = available_height * 0.4;
        let video_area_height = available_height * 0.6 - 10.0;
        
        ui.vertical(|ui| {
            ui.set_width(available_width);
            ui.set_height(available_height);
            
            // Stream deck style buttons (top 40%)
            ui.push_id("action_buttons_area", |ui| {
                ui.group(|ui| {
                    ui.set_min_height(button_area_height);
                    ui.set_max_height(button_area_height);
                    ui.set_width(available_width.min(300.0)); // Max width to prevent overflow
                    
                    ui.label(RichText::new("Quick Actions").strong());
                    ui.separator();
                    
                    // Calculate button sizes with proper margins
                    let group_padding = 5.0;
                    let button_spacing = 2.0;
                    let max_button_width = 50.0;
                    let usable_width = (available_width - group_padding * 2.0).min(150.0);
                    let button_width = ((usable_width - button_spacing * 2.0) / 3.0).min(max_button_width);
                    
                    let header_height = 30.0;
                    let usable_height = button_area_height - header_height - group_padding;
                    let button_height = ((usable_height - button_spacing * 2.0) / 3.0).min(35.0);
                    
                    // Create centered button grid
                    ui.vertical_centered(|ui| {
                        for row in 0..3 {
                            ui.horizontal(|ui| {
                                for col in 0..3 {
                                    let button_num = row * 3 + col + 1;
                                    ui.add_sized(
                                        [button_width, button_height],
                                        egui::Button::new(format!("{}", button_num))
                                    );
                                }
                            });
                        }
                    });
                });
            });
            
            ui.add_space(5.0);
            
            // Video player (bottom 60%)
            ui.push_id("main_video_area", |ui| {
                self.render_video_player(ui, video_area_height, available_width - 10.0, "Main Stream");
            });
        });
    }

    // Helper function to render a video player with 16:9 aspect ratio
    fn render_video_player(&self, ui: &mut egui::Ui, max_height: f32, max_width: f32, label: &str) {
        ui.group(|ui| {
            // Calculate 16:9 aspect ratio dimensions
            let aspect_ratio = 16.0 / 9.0;
            
            let (width, height) = if max_width / max_height > aspect_ratio {
                // Height-constrained
                let height = max_height - 10.0;
                let width = height * aspect_ratio;
                (width, height)
            } else {
                // Width-constrained
                let width = max_width - 10.0;
                let height = width / aspect_ratio;
                (width, height)
            };
            
            // Center the video player
            ui.allocate_ui_with_layout(
                Vec2::new(max_width, max_height),
                egui::Layout::centered_and_justified(egui::Direction::TopDown),
                |ui| {
                    ui.allocate_ui(Vec2::new(width, height), |ui| {
                        ui.group(|ui| {
                            ui.set_min_size(Vec2::new(width, height));
                            ui.centered_and_justified(|ui| {
                                ui.label(format!(
                                    "{}\n(CEF Embed)\n{}x{} (16:9)",
                                    label,
                                    width as i32,
                                    height as i32
                                ));
                            });
                        });
                    });
                    
                    // TODO: Report actual video size to business logic
                    // tracing::debug!("Video player '{}' size: {}x{}", label, width as i32, height as i32);
                },
            );
        });
    }

    // Helper function to render video grid
    fn render_video_grid(&self, ui: &mut egui::Ui, cols: usize, rows: usize) {
        let available_size = ui.available_size();
        let spacing = 5.0;
        
        let cell_width = (available_size.x - spacing * (cols - 1) as f32) / cols as f32;
        let cell_height = (available_size.y - spacing * (rows - 1) as f32) / rows as f32;
        
        egui::Grid::new("video_grid")
            .num_columns(cols)
            .spacing([spacing, spacing])
            .show(ui, |ui| {
                let mut player_num = 1;
                for row in 0..rows {
                    for col in 0..cols {
                        ui.allocate_ui(Vec2::new(cell_width, cell_height), |ui| {
                            self.render_video_player(
                                ui,
                                cell_height,
                                cell_width,
                                &format!("Stream {}", player_num),
                            );
                        });
                        player_num += 1;
                    }
                    if row < rows - 1 {
                        ui.end_row();
                    }
                }
            });
    }
}