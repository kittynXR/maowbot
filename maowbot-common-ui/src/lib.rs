pub mod chat;
pub mod grpc;
pub mod grpc_client;
pub mod state;
pub mod events;
pub mod settings;
pub mod commands;

pub use chat::{ChatState, ChatMessage, ChatEvent};
pub use grpc::SharedGrpcClient;
pub use grpc_client::GrpcClient;
pub use state::{AppState, LayoutSection};
pub use events::{UIEvent, AppEvent, ChatCommand};
pub use settings::{
    SettingsTab, ChatSide, StreamerListEntry, 
    UISettings, AudioSettings, StreamOverlaySettings
};
pub use commands::{CommandResult, CommandError};

use anyhow::Result;

// Trait for different rendering backends
pub trait UIRenderer {
    fn render_chat(&mut self, state: &ChatState) -> Result<()>;
    fn render_controls(&mut self, state: &AppState) -> Result<()>;
    fn handle_input(&mut self) -> Option<UIEvent>;
    fn should_quit(&self) -> bool;
}