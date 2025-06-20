// File: src/services/mod.rs

pub mod user_service;

pub mod message_service;
pub mod message_sender;
// Moved all Twitch-specific things into services/twitch.
pub mod twitch;
pub mod discord;
pub mod osc_toggle_service;

// New event handling system
pub mod event_context;
pub mod event_handler;
pub mod event_registry;
pub mod event_handlers;
pub mod event_pipeline;
pub mod event_pipeline_service;

// Re-export anything you want from twitch here, if desired, e.g.:
// pub use twitch::command_service::CommandService;
pub use twitch::command_service::CommandService;
pub use twitch::command_service::CommandResponse;
pub use twitch::redeem_service::RedeemService;
pub use twitch::eventsub_service::EventSubService;
pub use message_sender::MessageSender;
pub use message_sender::MessageResponse;