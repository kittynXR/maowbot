use std::sync::{Arc, Mutex};
use crate::chat::ChatState;

#[derive(Clone)]
pub struct AppState {
    pub chat_state: Arc<Mutex<ChatState>>,
    pub secondary_chat_state: Arc<Mutex<ChatState>>,
    pub overlay_running: Arc<Mutex<bool>>,
    pub grpc_connected: Arc<Mutex<bool>>,
    pub active_tab: Arc<Mutex<String>>,
    pub layout_order: Arc<Mutex<Vec<LayoutSection>>>,
    pub is_docked: Arc<Mutex<bool>>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum LayoutSection {
    LeftChat,
    TabArea,
    MainChat,
    RightPanel,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            chat_state: Arc::new(Mutex::new(ChatState::new())),
            secondary_chat_state: Arc::new(Mutex::new(ChatState::new())),
            overlay_running: Arc::new(Mutex::new(false)),
            grpc_connected: Arc::new(Mutex::new(false)),
            active_tab: Arc::new(Mutex::new("Multiview".to_string())),
            layout_order: Arc::new(Mutex::new(vec![
                LayoutSection::LeftChat,
                LayoutSection::TabArea,
                LayoutSection::MainChat,
                LayoutSection::RightPanel,
            ])),
            is_docked: Arc::new(Mutex::new(true)),
        }
    }
}