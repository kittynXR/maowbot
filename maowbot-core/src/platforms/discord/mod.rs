// File: src/platforms/discord/mod.rs

pub mod auth;
pub mod runtime;
pub mod songbird;

pub use auth::DiscordAuthenticator;
pub use runtime::DiscordPlatform;
