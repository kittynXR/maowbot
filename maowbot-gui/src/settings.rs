use egui::{Context, Ui};
use maowbot_ui::{
    SettingsTab, ChatSide, StreamerListEntry,
    UISettings, AudioSettings, StreamOverlaySettings
};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct Settings {
    pub ui_settings: UISettings,
    pub audio_settings: AudioSettings,
    pub stream_overlay_settings: StreamOverlaySettings,
    pub connection_url: String,
    pub connection_ca: String,
    pub active_tab: SettingsTab,
    pub show_settings: bool,
    temp_streamer_name: String,
    temp_streamer_platform: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            ui_settings: UISettings::default(),
            audio_settings: AudioSettings::default(),
            stream_overlay_settings: StreamOverlaySettings::default(),
            connection_url: std::env::var("MAOWBOT_GRPC_URL")
                .unwrap_or_else(|_| "https://localhost:9999".to_string()),
            connection_ca: std::env::var("MAOWBOT_GRPC_CA")
                .unwrap_or_else(|_| "certs/server.crt".to_string()),
            active_tab: SettingsTab::Connection,
            show_settings: false,
            temp_streamer_name: String::new(),
            temp_streamer_platform: String::new(),
        }
    }
}

impl Settings {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn render(&mut self, ctx: &Context) {
        if !self.show_settings {
            return;
        }

        let mut show_settings = self.show_settings;
        
        egui::Window::new("Settings")
            .id(egui::Id::new("settings_window"))
            .open(&mut show_settings)
            .resizable(true)
            .default_width(800.0)
            .default_height(500.0)
            .min_width(700.0)
            .min_height(400.0)
            .default_pos([400.0, 200.0])
            .collapsible(false)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.set_min_width(180.0);
                        ui.set_max_width(180.0);
                        self.render_tab_list(ui);
                    });
                    
                    ui.separator();
                    
                    ui.vertical(|ui| {
                        ui.set_min_width(500.0);
                        self.render_tab_content(ui);
                    });
                });
            });
            
        self.show_settings = show_settings;
    }

    fn render_tab_list(&mut self, ui: &mut Ui) {
        ui.heading("Settings");
        ui.add_space(5.0);
        ui.separator();
        ui.add_space(10.0);
        
        let tab_labels = |tab: &SettingsTab| -> &'static str {
            match tab {
                SettingsTab::Connection => "🔌 Connection",
                SettingsTab::General => "⚙️ General",
                SettingsTab::Platforms => "📱 Platforms",
                SettingsTab::CustomizeUI => "🎨 Customize UI",
                SettingsTab::Audio => "🔊 Audio",
                SettingsTab::StreamOverlay => "📺 Stream Overlay",
                SettingsTab::QuickActions => "⚡ Quick Actions",
                SettingsTab::Plugins => "🧩 Plugins",
                SettingsTab::About => "ℹ️ About",
            }
        };
        
        let mut move_up_index = None;
        let mut move_down_index = None;
        
        ui.with_layout(egui::Layout::top_down_justified(egui::Align::LEFT), |ui| {
            for (index, tab) in self.ui_settings.tab_order.clone().iter().enumerate() {
                ui.horizontal(|ui| {
                    // Show reorder controls if unlocked
                    if self.ui_settings.tab_reorder_unlocked {
                        ui.set_min_width(180.0);
                        
                        // Checkbox to enable/disable tab
                        let mut enabled = *self.ui_settings.tab_enabled.get(tab).unwrap_or(&true);
                        if ui.checkbox(&mut enabled, "").clicked() {
                            self.ui_settings.tab_enabled.insert(tab.clone(), enabled);
                        }
                        
                        // Up arrow
                        ui.add_enabled(index > 0, egui::Button::new("^").small())
                            .on_hover_text("Move up")
                            .clicked()
                            .then(|| move_up_index = Some(index));
                        
                        // Down arrow
                        ui.add_enabled(index < self.ui_settings.tab_order.len() - 1, egui::Button::new("v").small())
                            .on_hover_text("Move down")
                            .clicked()
                            .then(|| move_down_index = Some(index));
                        
                        ui.add_space(5.0);
                    }
                    
                    // Tab button
                    let is_selected = self.active_tab == *tab;
                    let label = tab_labels(tab);
                    let is_enabled = *self.ui_settings.tab_enabled.get(tab).unwrap_or(&true);
                    
                    // Disable tab if it's unchecked, but keep Customize UI always enabled
                    let can_click = is_enabled || *tab == SettingsTab::CustomizeUI;
                    
                    ui.add_enabled_ui(can_click, |ui| {
                        if !can_click {
                            ui.visuals_mut().override_text_color = Some(egui::Color32::from_gray(128));
                        }
                        
                        if ui.selectable_label(is_selected, label).clicked() && can_click {
                            self.active_tab = tab.clone();
                        }
                    });
                });
                ui.add_space(2.0);
            }
        });
        
        // Apply moves after iteration to avoid borrow issues
        if let Some(index) = move_up_index {
            self.ui_settings.tab_order.swap(index, index - 1);
        }
        if let Some(index) = move_down_index {
            self.ui_settings.tab_order.swap(index, index + 1);
        }
    }

    fn render_tab_content(&mut self, ui: &mut Ui) {
        match self.active_tab {
            SettingsTab::Connection => self.render_connection_tab(ui),
            SettingsTab::General => self.render_general_tab(ui),
            SettingsTab::Platforms => self.render_platforms_tab(ui),
            SettingsTab::CustomizeUI => self.render_customize_ui_tab(ui),
            SettingsTab::Audio => self.render_audio_tab(ui),
            SettingsTab::StreamOverlay => self.render_stream_overlay_tab(ui),
            SettingsTab::QuickActions => self.render_quick_actions_tab(ui),
            SettingsTab::Plugins => self.render_plugins_tab(ui),
            SettingsTab::About => self.render_about_tab(ui),
        }
    }

    fn render_connection_tab(&mut self, ui: &mut Ui) {
        ui.heading("Connection Settings");
        ui.separator();
        
        ui.label("Environment Variables:");
        ui.add_space(10.0);
        
        ui.horizontal(|ui| {
            ui.label("MAOWBOT_GRPC_URL:");
            ui.label(&self.connection_url);
        });
        
        ui.horizontal(|ui| {
            ui.label("MAOWBOT_GRPC_CA:");
            ui.label(&self.connection_ca);
        });
    }

    fn render_general_tab(&mut self, ui: &mut Ui) {
        ui.heading("General Settings");
        ui.separator();
        ui.label("General settings will be added here");
    }

    fn render_platforms_tab(&mut self, ui: &mut Ui) {
        ui.heading("Platform Settings");
        ui.separator();
        ui.label("Platform-specific settings will be added here");
    }

    fn render_customize_ui_tab(&mut self, ui: &mut Ui) {
        ui.heading("Customize UI");
        ui.separator();
        
        ui.add_space(10.0);
        ui.label("Chat Position Settings:");
        ui.add_space(5.0);
        
        ui.horizontal(|ui| {
            ui.checkbox(&mut self.ui_settings.main_chat_enabled, "Enable Main Chat");
        });
        
        if self.ui_settings.main_chat_enabled {
            ui.indent("main_chat_options", |ui| {
                ui.horizontal(|ui| {
                    ui.label("Position:");
                    ui.radio_value(&mut self.ui_settings.main_stream_chat_side, ChatSide::Left, "Left");
                    ui.radio_value(&mut self.ui_settings.main_stream_chat_side, ChatSide::Right, "Right");
                });
            });
        }
        
        ui.add_space(10.0);
        
        ui.horizontal(|ui| {
            ui.checkbox(&mut self.ui_settings.secondary_chat_enabled, "Enable Secondary Chat");
        });
        
        if self.ui_settings.secondary_chat_enabled {
            ui.indent("secondary_chat_options", |ui| {
                ui.horizontal(|ui| {
                    ui.label("Position:");
                    ui.radio_value(&mut self.ui_settings.secondary_stream_chat_side, ChatSide::Left, "Left");
                    ui.radio_value(&mut self.ui_settings.secondary_stream_chat_side, ChatSide::Right, "Right");
                });
            });
        }
        
        ui.add_space(10.0);
        
        ui.horizontal(|ui| {
            if ui.button("🔄 Swap Left and Right Halves").clicked() {
                self.ui_settings.swap_halves = !self.ui_settings.swap_halves;
            }
            ui.label(if self.ui_settings.swap_halves { "(Swapped)" } else { "(Normal)" });
        });
        
        ui.add_space(20.0);
        ui.separator();
        ui.add_space(10.0);
        
        // Tab reordering section
        ui.label("Settings Tab Order:");
        ui.add_space(5.0);
        
        ui.horizontal(|ui| {
            let lock_icon = if self.ui_settings.tab_reorder_unlocked { "🔓" } else { "🔒" };
            let lock_text = if self.ui_settings.tab_reorder_unlocked { 
                "Unlocked - Use arrows to reorder, uncheck to disable tabs" 
            } else { 
                "Locked - Click to unlock" 
            };
            
            if ui.button(format!("{} {}", lock_icon, lock_text)).clicked() {
                self.ui_settings.tab_reorder_unlocked = !self.ui_settings.tab_reorder_unlocked;
            }
        });
        
        if self.ui_settings.tab_reorder_unlocked {
            ui.add_space(5.0);
            ui.label(egui::RichText::new("Note: Customize UI tab cannot be disabled for safety").small().weak());
        }
        
        ui.add_space(20.0);
        ui.separator();
        ui.add_space(10.0);
        
        ui.label("Multistream Viewer Streamer List:");
        ui.add_space(5.0);
        
        // Add new streamer
        ui.horizontal(|ui| {
            ui.label("Name:");
            ui.text_edit_singleline(&mut self.temp_streamer_name);
            ui.label("Platform:");
            ui.text_edit_singleline(&mut self.temp_streamer_platform);
            
            if ui.button("Add").clicked() && !self.temp_streamer_name.is_empty() {
                let new_entry = StreamerListEntry {
                    id: uuid::Uuid::new_v4().to_string(),
                    name: self.temp_streamer_name.clone(),
                    platform: self.temp_streamer_platform.clone(),
                    enabled: true,
                };
                self.ui_settings.multistream_viewer_list.push(new_entry);
                self.temp_streamer_name.clear();
                self.temp_streamer_platform.clear();
            }
        });
        
        ui.add_space(10.0);
        
        // List existing streamers
        let mut to_remove = None;
        for (index, streamer) in self.ui_settings.multistream_viewer_list.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                ui.checkbox(&mut streamer.enabled, "");
                ui.label(&streamer.name);
                ui.label(format!("({})", streamer.platform));
                
                if ui.small_button("🗑️").clicked() {
                    to_remove = Some(index);
                }
            });
        }
        
        if let Some(index) = to_remove {
            self.ui_settings.multistream_viewer_list.remove(index);
        }
    }

    fn render_audio_tab(&mut self, ui: &mut Ui) {
        ui.heading("Audio Settings");
        ui.separator();
        
        ui.add_space(10.0);
        
        // Volume controls
        ui.label("Volume Controls:");
        ui.add_space(5.0);
        
        ui.horizontal(|ui| {
            ui.label("Master Volume:");
            ui.add(egui::Slider::new(&mut self.audio_settings.master_volume, 0.0..=1.0)
                .show_value(true)
                .custom_formatter(|n, _| format!("{:.0}%", n * 100.0)));
        });
        
        ui.horizontal(|ui| {
            ui.label("Alert Volume:");
            ui.add(egui::Slider::new(&mut self.audio_settings.alert_volume, 0.0..=1.0)
                .show_value(true)
                .custom_formatter(|n, _| format!("{:.0}%", n * 100.0)));
            ui.checkbox(&mut self.audio_settings.mute_alerts, "Mute");
        });
        
        ui.horizontal(|ui| {
            ui.label("TTS Volume:");
            ui.add(egui::Slider::new(&mut self.audio_settings.tts_volume, 0.0..=1.0)
                .show_value(true)
                .custom_formatter(|n, _| format!("{:.0}%", n * 100.0)));
            ui.checkbox(&mut self.audio_settings.mute_tts, "Mute");
        });
        
        ui.add_space(20.0);
        ui.separator();
        ui.add_space(10.0);
        
        // Audio device selection
        ui.label("Audio Device:");
        ui.add_space(5.0);
        
        egui::ComboBox::from_label("")
            .selected_text(&self.audio_settings.audio_device)
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut self.audio_settings.audio_device, "Default".to_string(), "Default");
                ui.selectable_value(&mut self.audio_settings.audio_device, "Speakers".to_string(), "Speakers");
                ui.selectable_value(&mut self.audio_settings.audio_device, "Headphones".to_string(), "Headphones");
                ui.selectable_value(&mut self.audio_settings.audio_device, "VB-Audio Cable".to_string(), "VB-Audio Cable");
            });
    }

    fn render_stream_overlay_tab(&mut self, ui: &mut Ui) {
        ui.heading("Stream Overlay Settings");
        ui.separator();
        
        ui.add_space(10.0);
        
        // Chat overlay settings
        ui.checkbox(&mut self.stream_overlay_settings.show_chat, "Show Chat Overlay");
        
        if self.stream_overlay_settings.show_chat {
            ui.add_space(5.0);
            ui.indent("chat_settings", |ui| {
                ui.horizontal(|ui| {
                    ui.label("Opacity:");
                    ui.add(egui::Slider::new(&mut self.stream_overlay_settings.chat_opacity, 0.0..=1.0)
                        .show_value(true)
                        .custom_formatter(|n, _| format!("{:.0}%", n * 100.0)));
                });
                
                ui.add_space(5.0);
                ui.label("Position:");
                
                ui.horizontal(|ui| {
                    ui.label("X:");
                    ui.add(egui::DragValue::new(&mut self.stream_overlay_settings.chat_position_x)
                        .speed(1.0)
                        .suffix("px"));
                    ui.label("Y:");
                    ui.add(egui::DragValue::new(&mut self.stream_overlay_settings.chat_position_y)
                        .speed(1.0)
                        .suffix("px"));
                });
                
                ui.horizontal(|ui| {
                    ui.label("Width:");
                    ui.add(egui::DragValue::new(&mut self.stream_overlay_settings.chat_width)
                        .speed(1.0)
                        .clamp_range(100.0..=1000.0)
                        .suffix("px"));
                    ui.label("Height:");
                    ui.add(egui::DragValue::new(&mut self.stream_overlay_settings.chat_height)
                        .speed(1.0)
                        .clamp_range(100.0..=1200.0)
                        .suffix("px"));
                });
            });
        }
        
        ui.add_space(20.0);
        ui.separator();
        ui.add_space(10.0);
        
        // Alert overlay settings
        ui.checkbox(&mut self.stream_overlay_settings.show_alerts, "Show Alert Overlay");
        
        if self.stream_overlay_settings.show_alerts {
            ui.add_space(5.0);
            ui.indent("alert_settings", |ui| {
                ui.horizontal(|ui| {
                    ui.label("Position:");
                    egui::ComboBox::from_id_source("alert_position")
                        .selected_text(&self.stream_overlay_settings.alert_position)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut self.stream_overlay_settings.alert_position, "Top Center".to_string(), "Top Center");
                            ui.selectable_value(&mut self.stream_overlay_settings.alert_position, "Top Left".to_string(), "Top Left");
                            ui.selectable_value(&mut self.stream_overlay_settings.alert_position, "Top Right".to_string(), "Top Right");
                            ui.selectable_value(&mut self.stream_overlay_settings.alert_position, "Center".to_string(), "Center");
                            ui.selectable_value(&mut self.stream_overlay_settings.alert_position, "Bottom Center".to_string(), "Bottom Center");
                        });
                });
                
                ui.horizontal(|ui| {
                    ui.label("Duration:");
                    ui.add(egui::Slider::new(&mut self.stream_overlay_settings.alert_duration, 1.0..=10.0)
                        .show_value(true)
                        .suffix("s"));
                });
            });
        }
    }

    fn render_quick_actions_tab(&mut self, ui: &mut Ui) {
        ui.heading("Quick Actions");
        ui.separator();
        ui.label("Quick action settings will be added here");
    }

    fn render_plugins_tab(&mut self, ui: &mut Ui) {
        ui.heading("Plugins");
        ui.separator();
        ui.label("Plugin management will be added here");
    }

    fn render_about_tab(&mut self, ui: &mut Ui) {
        ui.heading("About MaowBot");
        ui.separator();
        ui.label("MaowBot GUI");
        ui.label("Version: 0.1.0");
        ui.add_space(10.0);
        ui.label("A multi-platform streaming bot with VRChat integration");
    }

    pub fn toggle(&mut self) {
        self.show_settings = !self.show_settings;
    }

    pub fn is_open(&self) -> bool {
        self.show_settings
    }
    
    pub fn get_ui_settings(&self) -> UISettings {
        self.ui_settings.clone()
    }
}