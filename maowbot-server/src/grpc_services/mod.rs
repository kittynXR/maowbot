// gRPC service implementations

pub mod user_service;
pub mod credential_service;
pub mod platform_service;
pub mod plugin_service;
pub mod config_service;
pub mod ai_service;
pub mod command_service;
pub mod redeem_service;
pub mod twitch_service;
pub mod discord_service;
pub mod vrchat_service;
pub mod osc_service;
pub mod autostart_service;

// Re-export service implementations
pub use user_service::UserServiceImpl;
pub use credential_service::CredentialServiceImpl;
pub use platform_service::PlatformServiceImpl;
pub use plugin_service::PluginServiceImpl;
pub use config_service::ConfigServiceImpl;
pub use ai_service::AiServiceImpl;
pub use command_service::CommandServiceImpl;
pub use redeem_service::RedeemServiceImpl;
pub use twitch_service::TwitchServiceImpl;
pub use discord_service::DiscordServiceImpl;
pub use vrchat_service::VRChatServiceImpl;
pub use osc_service::OscServiceImpl;
pub use autostart_service::AutostartServiceImpl;