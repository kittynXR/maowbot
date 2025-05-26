use maowbot_ui::{AppState, ChatState, ChatMessage};
use std::ffi::CString;

pub struct ImGuiOverlayRenderer {
    is_dashboard: bool,
    input_buffer: [u8; 256],
    message_sent: bool,
}

impl ImGuiOverlayRenderer {
    pub fn new(is_dashboard: bool) -> Self {
        Self {
            is_dashboard,
            input_buffer: [0; 256],
            message_sent: false,
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
}