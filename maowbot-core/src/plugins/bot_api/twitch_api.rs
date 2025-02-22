//! plugins/bot_api/twitch_api.rs
//!
//! Sub-trait for Twitch IRC–specific methods.

use crate::Error;
use async_trait::async_trait;

/// Sub-trait focusing on Twitch IRC channel joining, parting, sending messages, etc.
#[async_trait]
pub trait TwitchApi: Send + Sync {
    /// Joins a Twitch IRC channel (e.g. "#somechannel") using the specified account credentials.
    async fn join_twitch_irc_channel(&self, account_name: &str, channel: &str) -> Result<(), Error>;

    /// Leaves a Twitch IRC channel.
    async fn part_twitch_irc_channel(&self, account_name: &str, channel: &str) -> Result<(), Error>;

    /// Sends a chat message to a Twitch IRC channel.
    async fn send_twitch_irc_message(&self, account_name: &str, channel: &str, text: &str) -> Result<(), Error>;
}