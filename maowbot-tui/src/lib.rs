// maowbot-tui/src/lib.rs

mod tui_module;
pub mod tui_module_simple;
pub mod commands;
pub mod help;
pub mod test_harness;
pub mod completion;
pub mod unified_completer;

pub use tui_module::TuiModule;
pub use tui_module_simple::SimpleTuiModule;

// Export adapters for use in main.rs
pub use commands::{user_adapter, platform_adapter, twitch_adapter, test_grpc};