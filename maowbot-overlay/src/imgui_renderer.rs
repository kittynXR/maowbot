use maowbot_common_ui::{AppState, ChatState, ChatMessage};
use maowbot_common_ui::settings::{StreamOverlaySettings, UISettings, AudioSettings};
use std::ffi::CString;
use crate::ffi::{DashboardState, OverlaySettingsFFI};

pub struct ImGuiOverlayRenderer {
    is_dashboard: bool,
    input_buffer: [u8; 256],
    message_sent: bool,
    dashboard_state: DashboardState,
}

impl ImGuiOverlayRenderer {
    pub fn new(is_dashboard: bool) -> Self {
        Self {
            is_dashboard,
            input_buffer: [0; 256],
            message_sent: false,
            dashboard_state: DashboardState {
                show_settings: false,
                current_tab: 0,
            },
        }
    }

    pub fn update_state(&mut self, state: &AppState) {
        let chat_state = state.chat_state.lock().unwrap();
        let ffi_messages = chat_state.to_ffi_messages();

        unsafe {
            crate::ffi::imgui_update_chat_state(
                if ffi_messages.is_empty() {
                    std::ptr::null()
                } else {
                    ffi_messages.as_ptr() as *const u8
                },
                ffi_messages.len(),
                self.input_buffer.as_mut_ptr(),
                self.input_buffer.len(),
            );
        }
    }

    pub fn get_sent_message(&mut self) -> Option<String> {
        self.input_buffer.fill(0);
        let sent = unsafe {
            crate::ffi::imgui_get_sent_message(
                self.input_buffer.as_mut_ptr(),
                256
            )
        };

        if sent {
            if let Ok(text) = std::str::from_utf8(&self.input_buffer) {
                if let Some(text) = text.trim_end_matches('\0').trim().to_string().into() {
                    if !text.is_empty() {
                        return Some(text);
                    }
                }
            }
        }
        None
    }
    
    pub fn update_dashboard_state(&mut self, show_settings: bool, settings: &StreamOverlaySettings) {
        if self.is_dashboard {
            // Update local dashboard state
            self.dashboard_state.show_settings = show_settings;
            
            // Send dashboard state to C++
            unsafe {
                crate::ffi::imgui_update_dashboard_state(&self.dashboard_state);
            }
            
            // Convert and send overlay settings
            let ffi_settings = OverlaySettingsFFI {
                show_chat: settings.show_chat,
                chat_opacity: settings.chat_opacity,
                chat_position_x: settings.chat_position_x,
                chat_position_y: settings.chat_position_y,
                chat_width: settings.chat_width,
                chat_height: settings.chat_height,
                show_alerts: settings.show_alerts,
                alert_duration: settings.alert_duration,
            };
            
            unsafe {
                crate::ffi::imgui_update_overlay_settings(&ffi_settings);
            }
        }
    }
    
    pub fn check_dashboard_state_change(&mut self) -> bool {
        if self.is_dashboard {
            let mut new_state = DashboardState {
                show_settings: false,
                current_tab: 0,
            };
            
            let changed = unsafe {
                crate::ffi::imgui_get_dashboard_state(&mut new_state)
            };
            
            if changed {
                self.dashboard_state = new_state;
            }
            
            changed
        } else {
            false
        }
    }
    
    pub fn get_dashboard_state(&self) -> &DashboardState {
        &self.dashboard_state
    }
}