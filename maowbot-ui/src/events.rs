use crate::chat::ChatEvent;

#[derive(Clone)]
pub enum UIEvent {
    SendMessage(String),
    ToggleKeyboard,
    RestartOverlay,
    StopOverlay,
    StartOverlay,
    OpenWebView(String),
    Quit,
    Undock,
    Dock,
}

pub enum AppEvent {
    Chat(ChatEvent),
    OverlayStatusChanged(bool),
    GrpcStatusChanged(bool),
    Shutdown,
}

pub enum ChatCommand {
    SendMessage(String),
}