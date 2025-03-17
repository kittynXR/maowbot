// File: src/services/mod.rs

pub mod user_service;

pub mod message_service;
// Moved all Twitch-specific things into services/twitch.
pub mod twitch;
pub mod discord;

// Re-export anything you want from twitch here, if desired, e.g.:
// pub use twitch::command_service::CommandService;
pub use twitch::command_service::CommandService;
pub use twitch::command_service::CommandResponse;
pub use twitch::redeem_service::RedeemService;
pub use twitch::eventsub_service::EventSubService;