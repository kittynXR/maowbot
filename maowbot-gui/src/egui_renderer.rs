use crossbeam_channel::Sender;
use egui::{Color32, RichText, ScrollArea, TextEdit, Vec2, Rect};
use maowbot_common_ui::{AppState, UIEvent, LayoutSection};
use maowbot_common_ui::events::ChatCommand;
use std::sync::{Arc, Mutex};

use crate::layout_constants::*;
use crate::process_manager::ProcessManager;
use crate::settings::Settings;
use crate::WindowMode;

pub struct EguiRenderer {
    input_buffer: String,
    secondary_input_buffer: String,
    show_settings: bool,
    window_mode: WindowMode,
    settings: Arc<Mutex<Settings>>,
}

impl EguiRenderer {
    pub fn new(window_mode: WindowMode) -> Self {
        Self {
            input_buffer: String::new(),
            secondary_input_buffer: String::new(),
            show_settings: false,
            window_mode,
            settings: Arc::new(Mutex::new(Settings::new())),
        }
    }
    
    pub fn new_with_settings(window_mode: WindowMode, settings: Arc<Mutex<Settings>>) -> Self {
        Self {
            input_buffer: String::new(),
            secondary_input_buffer: String::new(),
            show_settings: false,
            window_mode,
            settings,
        }
    }
    
    pub fn get_settings(&self) -> Arc<Mutex<Settings>> {
        self.settings.clone()
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
        egui::CentralPanel::default()
            .frame(egui::Frame::default().inner_margin(egui::Margin {
                left: CONTENT_MARGIN as i8,
                right: (CONTENT_MARGIN + 5.0) as i8,  // Extra right margin
                top: CONTENT_MARGIN as i8,
                bottom: (CONTENT_MARGIN + 5.0) as i8, // Extra bottom margin
            }))
            .show(ctx, |ui| {
            ui.horizontal_top(|ui| {
                ui.set_height(ui.available_height());
                
                let total_width = ui.available_width();
                let ui_settings = {
                    let settings = self.settings.lock().unwrap();
                    settings.get_ui_settings().clone()
                };
                
                // Check if secondary chat is enabled
                if !ui_settings.secondary_chat_enabled {
                    // Only show tabs area
                    ui.allocate_ui_with_layout(
                        egui::vec2(total_width, ui.available_height()),
                        egui::Layout::top_down(egui::Align::LEFT),
                        |ui| {
                            ui.set_width(total_width);
                            self.render_tab_area(ui, state);
                        },
                    );
                    return;
                }
                
                let separator_width = 2.0;
                
                // Apply same constraints as main chat
                let min_chat_width = 280.0;
                let ideal_chat_width = 340.0;
                let max_chat_width = 400.0;
                
                // Calculate tabs minimum width requirement
                let min_tabs_width = 400.0; // Minimum for tabs to be useful
                
                // Determine chat width based on available space
                let final_chat_width = if total_width - separator_width >= ideal_chat_width + min_tabs_width {
                    // Plenty of space, use ideal width
                    ideal_chat_width
                } else if total_width - separator_width >= min_chat_width + min_tabs_width {
                    // Limited space, use minimum
                    min_chat_width
                } else {
                    // Very limited space, use percentage
                    (total_width * 0.3).max(200.0)
                }.min(max_chat_width); // Never exceed max
                
                let tabs_width = total_width - final_chat_width - separator_width;

                // Determine order based on chat position setting
                use crate::settings::ChatSide;
                if ui_settings.secondary_stream_chat_side == ChatSide::Left {
                    // Secondary chat on left
                    ui.allocate_ui_with_layout(
                        egui::vec2(final_chat_width, ui.available_height()),
                        egui::Layout::top_down(egui::Align::LEFT),
                        |ui| {
                            ui.set_width(final_chat_width);
                            ui.set_height(ui.available_height());
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
                } else {
                    // Tabs area first
                    ui.allocate_ui_with_layout(
                        egui::vec2(tabs_width, ui.available_height()),
                        egui::Layout::top_down(egui::Align::LEFT),
                        |ui| {
                            ui.set_width(tabs_width);
                            self.render_tab_area(ui, state);
                        },
                    );

                    ui.add(egui::Separator::default().vertical());

                    // Secondary chat on right
                    ui.allocate_ui_with_layout(
                        egui::vec2(final_chat_width, ui.available_height()),
                        egui::Layout::top_down(egui::Align::LEFT),
                        |ui| {
                            ui.set_width(final_chat_width);
                            ui.set_height(ui.available_height());
                            let (dummy_tx, _) = crossbeam_channel::unbounded();
                            self.render_left_chat(ui, state, &dummy_tx);
                        },
                    );
                }
            });
        });

        if should_dock {
            *state.is_docked.lock().unwrap() = true;
            // Request repaint to ensure main window updates
            ctx.request_repaint();
            
            // On Windows, we need to wake up the main window
            // Send a viewport command to the main window to force it to process events
            let main_viewport = egui::ViewportId::ROOT;
            ctx.send_viewport_cmd_to(main_viewport, egui::ViewportCommand::Focus);
            
            // Send close command to this window
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
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
                    self.settings.lock().unwrap().toggle();
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
        self.settings.lock().unwrap().render(ctx);

        // Main content area with 4 sections
        egui::CentralPanel::default()
            .frame(egui::Frame::default().inner_margin(egui::Margin {
                left: CONTENT_MARGIN as i8,
                right: (CONTENT_MARGIN + 5.0) as i8,  // Extra right margin
                top: CONTENT_MARGIN as i8,
                bottom: (CONTENT_MARGIN + 5.0) as i8, // Extra bottom margin
            }))
            .show(ctx, |ui| {
            // Get actual separator width from egui
            let separator_spacing = ui.spacing().item_spacing.x;
            let separator_count = 3;
            let total_separator_width = separator_count as f32 * separator_spacing;
            
            let total_width = ui.available_width();
            let available_height = ui.available_height();
            let usable_width = total_width - total_separator_width;
            
            // Get UI settings to check chat visibility
            let ui_settings = self.settings.lock().unwrap().get_ui_settings().clone();
            
            // Fixed widths for chats (0 if disabled)
            let main_chat_width = if ui_settings.main_chat_enabled { 340.0 } else { 0.0 };  // Standard Twitch chat width
            let left_chat_width = if ui_settings.secondary_chat_enabled { 280.0 } else { 0.0 };  // Slightly smaller secondary chat
            
            // Calculate right panel width based on video needs
            let video_height = available_height * 0.6;
            let min_video_width = 200.0;
            let ideal_video_width = video_height * (16.0 / 9.0);
            let max_right_width = usable_width * 0.3;
            let right_panel_width = ideal_video_width.clamp(min_video_width, max_right_width);
            
            // Tab area gets remaining space
            let tab_area_width = usable_width - left_chat_width - main_chat_width - right_panel_width;
            
            // Debug output
            tracing::debug!(
                "Layout: total={:.0}, usable={:.0}, left={:.0}, tab={:.0}, main={:.0}, right={:.0}, separators={:.0}",
                total_width, usable_width, left_chat_width, tab_area_width, main_chat_width, right_panel_width, total_separator_width
            );
            
            // Ensure minimum widths
            let min_tab_width = 300.0;
            let section_widths = if tab_area_width < min_tab_width {
                // Scale everything proportionally
                let scale = usable_width / (left_chat_width + min_tab_width + main_chat_width + right_panel_width);
                [
                    left_chat_width * scale,
                    min_tab_width * scale,
                    main_chat_width * scale,
                    right_panel_width * scale,
                ]
            } else {
                [
                    left_chat_width,
                    tab_area_width,
                    main_chat_width,
                    right_panel_width,
                ]
            };

            ui.horizontal_top(|ui| {
                let container_height = ui.available_height();
                ui.set_height(container_height);
                
                // Apply UI settings to determine layout order
                let mut layout_order = vec![];
                let ui_settings = self.settings.lock().unwrap().get_ui_settings().clone();
                
                // The default layout has two halves:
                // Left half: Secondary Chat + Tab Area
                // Right half: Main Chat + Right Panel
                
                if ui_settings.swap_halves {
                    // Swapped: Right half first, then left half
                    // Determine main chat position within right half
                    use crate::settings::ChatSide;
                    if ui_settings.main_chat_enabled {
                        if ui_settings.main_stream_chat_side == ChatSide::Left {
                            layout_order.push(LayoutSection::MainChat);
                            layout_order.push(LayoutSection::RightPanel);
                        } else {
                            layout_order.push(LayoutSection::RightPanel);
                            layout_order.push(LayoutSection::MainChat);
                        }
                    } else {
                        // Main chat disabled, only show right panel
                        layout_order.push(LayoutSection::RightPanel);
                    }
                    
                    // Then secondary chat position within left half
                    if ui_settings.secondary_chat_enabled {
                        if ui_settings.secondary_stream_chat_side == ChatSide::Left {
                            layout_order.push(LayoutSection::LeftChat);
                            layout_order.push(LayoutSection::TabArea);
                        } else {
                            layout_order.push(LayoutSection::TabArea);
                            layout_order.push(LayoutSection::LeftChat);
                        }
                    } else {
                        // Secondary chat disabled, only show tab area
                        layout_order.push(LayoutSection::TabArea);
                    }
                } else {
                    // Normal: Left half first, then right half
                    // Determine secondary chat position within left half
                    use crate::settings::ChatSide;
                    if ui_settings.secondary_chat_enabled {
                        if ui_settings.secondary_stream_chat_side == ChatSide::Left {
                            layout_order.push(LayoutSection::LeftChat);
                            layout_order.push(LayoutSection::TabArea);
                        } else {
                            layout_order.push(LayoutSection::TabArea);
                            layout_order.push(LayoutSection::LeftChat);
                        }
                    } else {
                        // Secondary chat disabled, only show tab area
                        layout_order.push(LayoutSection::TabArea);
                    }
                    
                    // Then main chat position within right half
                    if ui_settings.main_chat_enabled {
                        if ui_settings.main_stream_chat_side == ChatSide::Left {
                            layout_order.push(LayoutSection::MainChat);
                            layout_order.push(LayoutSection::RightPanel);
                        } else {
                            layout_order.push(LayoutSection::RightPanel);
                            layout_order.push(LayoutSection::MainChat);
                        }
                    } else {
                        // Main chat disabled, only show right panel
                        layout_order.push(LayoutSection::RightPanel);
                    }
                }
                
                // Update the state's layout order
                *state.layout_order.lock().unwrap() = layout_order.clone();
                
                for (i, section) in layout_order.iter().enumerate() {
                    if i > 0 {
                        ui.separator();
                    }

                    let width = match section {
                        LayoutSection::LeftChat => section_widths[0],
                        LayoutSection::TabArea => section_widths[1],
                        LayoutSection::MainChat => section_widths[2],
                        LayoutSection::RightPanel => section_widths[3],
                    };

                    ui.allocate_ui_with_layout(
                        egui::vec2(width, container_height),
                        egui::Layout::top_down(egui::Align::LEFT),
                        |ui| {
                            // Create a child UI with margins to prevent overflow
                            let margin = 2.0;
                            let child_rect = ui.available_rect_before_wrap();
                            let child_rect = Rect::from_min_size(
                                child_rect.min + Vec2::new(margin, margin),
                                child_rect.size() - Vec2::new(margin * 2.0, margin * 2.0)
                            );
                            
                            ui.allocate_ui_at_rect(child_rect, |ui| {
                                match section {
                                    LayoutSection::LeftChat => self.render_left_chat(ui, state, command_tx),
                                    LayoutSection::TabArea => self.render_tab_area(ui, state),
                                    LayoutSection::MainChat => self.render_main_chat(ui, state, command_tx),
                                    LayoutSection::RightPanel => self.render_right_panel(ui),
                                }
                            });
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
        egui::CentralPanel::default()
            .frame(egui::Frame::default().inner_margin(egui::Margin {
                left: CONTENT_MARGIN as i8,
                right: (CONTENT_MARGIN + 5.0) as i8,  // Extra right margin
                top: CONTENT_MARGIN as i8,
                bottom: (CONTENT_MARGIN + 5.0) as i8, // Extra bottom margin
            }))
            .show(ctx, |ui| {
            ui.horizontal_top(|ui| {
                ui.set_height(ui.available_height());
                
                let total_width = ui.available_width();
                let ui_settings = self.settings.lock().unwrap().get_ui_settings().clone();
                
                // Check if main chat is enabled
                if !ui_settings.main_chat_enabled {
                    // Only show right panel
                    ui.allocate_ui_with_layout(
                        egui::vec2(total_width, ui.available_height()),
                        egui::Layout::top_down(egui::Align::LEFT),
                        |ui| {
                            ui.set_width(total_width);
                            self.render_right_panel(ui);
                        },
                    );
                    return;
                }
                
                let separator_width = 2.0;
                
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

                // Determine order based on chat position setting
                use crate::settings::ChatSide;
                if ui_settings.main_stream_chat_side == ChatSide::Left {
                    // Main chat on left
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
                } else {
                    // Right panel first
                    ui.allocate_ui_with_layout(
                        egui::vec2(final_right_width, ui.available_height()),
                        egui::Layout::top_down(egui::Align::LEFT),
                        |ui| {
                            ui.set_width(final_right_width);
                            self.render_right_panel(ui);
                        },
                    );

                    ui.add(egui::Separator::default().vertical());

                    // Main chat on right
                    ui.allocate_ui_with_layout(
                        egui::vec2(final_chat_width, ui.available_height()),
                        egui::Layout::top_down(egui::Align::LEFT),
                        |ui| {
                            ui.set_width(final_chat_width);
                            self.render_main_chat(ui, state, command_tx);
                        },
                    );
                }
            });
        });

        result
    }

    fn render_left_chat(&mut self, ui: &mut egui::Ui, state: &AppState, _command_tx: &Sender<ChatCommand>) {
        let available_height = ui.available_height();
        
        // Secondary chat area
        ui.vertical(|ui| {
            ui.set_height(available_height);
            ui.set_width(ui.available_width());
            
            ui.label(RichText::new("Secondary Chat").strong());
            ui.separator();
            
            // Account for vertical container padding
            let chat_height = available_height - CHAT_CHROME_HEIGHT - VERTICAL_CONTAINER_PADDING;
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
    }

    fn render_main_chat(&mut self, ui: &mut egui::Ui, state: &AppState, command_tx: &Sender<ChatCommand>) {
        let available_height = ui.available_height();
        
        ui.vertical(|ui| {
            ui.set_height(available_height);
            ui.set_width(ui.available_width());
            
            ui.label(RichText::new("Main Stream Chat").strong());
            ui.separator();
            
            // Chat area - account for vertical container padding
            let chat_height = available_height - CHAT_CHROME_HEIGHT - VERTICAL_CONTAINER_PADDING;
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
        
        let section_spacing = 5.0;
        let button_area_height = (available_height - section_spacing) * 0.4;
        let video_area_height = (available_height - section_spacing) * 0.6;
        
        // Stream deck style buttons (top 40%)
        ui.push_id("action_buttons_area", |ui| {
                ui.group(|ui| {
                    ui.set_height(button_area_height);
                    ui.set_width(available_width);
                    
                    ui.label(RichText::new("Quick Actions").strong());
                    ui.separator();
                    
                    // Calculate button sizes with proper margins
                    let button_spacing = 2.0;
                    let max_button_width = 50.0;
                    let usable_width = (ui.available_width() - GROUP_WIDGET_MARGIN).min(150.0);
                    let button_width = ((usable_width - button_spacing * 2.0) / 3.0).min(max_button_width);
                    
                    let header_height = HEADER_HEIGHT + SEPARATOR_HEIGHT;
                    let usable_height = button_area_height - header_height - GROUP_WIDGET_MARGIN;
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
        
        ui.add_space(section_spacing);
        
        // Video player (bottom 60%)
        ui.push_id("main_video_area", |ui| {
            self.render_video_player(ui, video_area_height, available_width, "Main Stream");
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
        // Don't use group here - it adds extra borders that overflow
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