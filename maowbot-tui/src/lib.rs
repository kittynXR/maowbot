// maowbot-tui/src/lib.rs

mod tui_module;
pub mod commands;
pub mod help;

pub use tui_module::TuiModule;

// Export adapters for use in main.rs
pub use commands::{user_adapter, platform_adapter, test_grpc};