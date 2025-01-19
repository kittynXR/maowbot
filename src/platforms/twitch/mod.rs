// File: src/platforms/twitch/mod.rs

pub mod auth;
pub mod runtime;

// Re-export whichever items you want publicly
pub use auth::TwitchAuthenticator;
pub use runtime::TwitchPlatform;
