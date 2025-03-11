//! plugins/bot_api/mod.rs
//!
//! Defines a “master” BotApi trait that extends several sub-traits.

pub mod plugin_api;
pub mod user_api;
pub mod credentials_api;
pub mod platform_api;
pub mod twitch_api;
pub mod vrchat_api;
pub mod command_api;
pub mod redeem_api;
pub mod osc_api;

// Bring sub-traits into scope:
use plugin_api::PluginApi;
use user_api::UserApi;
use credentials_api::CredentialsApi;
use platform_api::PlatformApi;
use twitch_api::TwitchApi;
use vrchat_api::VrchatApi;
use command_api::CommandApi;
use redeem_api::RedeemApi;
use osc_api::OscApi;

/// The new umbrella trait `BotApi` that extends all the sub-traits.
/// Any type implementing all those sub-traits automatically implements `BotApi`.
pub trait BotApi:
PluginApi
+ UserApi
+ CredentialsApi
+ PlatformApi
+ TwitchApi
+ VrchatApi
+ CommandApi
+ RedeemApi
+ OscApi
{
}

impl<T> BotApi for T
where
    T: PluginApi
    + UserApi
    + CredentialsApi
    + PlatformApi
    + TwitchApi
    + VrchatApi
    + CommandApi
    + RedeemApi
    + OscApi
{
    // No extra methods; it’s just a “marker” for convenience.
}