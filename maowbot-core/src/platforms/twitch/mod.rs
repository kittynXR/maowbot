// File: maowbot-core/src/platforms/twitch/mod.rs

pub mod auth;
pub mod runtime;
pub mod client;

// NEW: add a requests submodule directory
pub mod requests;

pub use auth::TwitchAuthenticator;
pub use runtime::TwitchPlatform;
