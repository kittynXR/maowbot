use std::sync::{Arc, Mutex};
use crate::chat::ChatState;

#[derive(Clone)]
pub struct AppState {
    pub chat_state: Arc<Mutex<ChatState>>,
    pub overlay_running: Arc<Mutex<bool>>,
    pub grpc_connected: Arc<Mutex<bool>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            chat_state: Arc::new(Mutex::new(ChatState::new())),
            overlay_running: Arc::new(Mutex::new(false)),
            grpc_connected: Arc::new(Mutex::new(false)),
        }
    }
}