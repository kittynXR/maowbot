//! plugins/manager/mod.rs
//!
//! This mod re-exports the primary `PluginManager` struct and submodules.

pub mod core;
pub mod plugin_api_impl;
pub mod user_api_impl;
pub mod credentials_api_impl;
pub mod platform_api_impl;
pub mod twitch_api_impl;
pub mod vrchat_api_impl;
pub mod command_api_impl;
pub mod redeem_api_impl;

pub mod osc_api_impl;
pub mod drip_api_impl;
pub mod bot_config_api_impl;
pub mod discord_api_impl;
pub mod ai_api_impl;
pub mod autostart_api_impl;

// re-export the manager
pub use core::PluginManager;
