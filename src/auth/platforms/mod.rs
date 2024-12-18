// src/auth/platforms/mod.rs
pub mod twitch;
pub mod discord;
pub mod vrchat;

pub use twitch::TwitchAuthenticator;
pub use discord::DiscordAuthenticator;
pub use vrchat::VRChatAuthenticator;