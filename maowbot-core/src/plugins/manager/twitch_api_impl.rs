//! plugins/manager/twitch_api_impl.rs
//!
//! Implements TwitchApi for PluginManager (IRC join, part, send).

use crate::Error;
use crate::plugins::bot_api::twitch_api::TwitchApi;
use crate::plugins::manager::core::PluginManager;
use async_trait::async_trait;

#[async_trait]
impl TwitchApi for PluginManager {
    async fn join_twitch_irc_channel(&self, account_name: &str, channel: &str) -> Result<(), Error> {
        self.platform_manager.join_twitch_irc_channel(account_name, channel).await
    }

    async fn part_twitch_irc_channel(&self, account_name: &str, channel: &str) -> Result<(), Error> {
        self.platform_manager.part_twitch_irc_channel(account_name, channel).await
    }

    async fn send_twitch_irc_message(&self, account_name: &str, channel: &str, text: &str) -> Result<(), Error> {
        self.platform_manager.send_twitch_irc_message(account_name, channel, text).await
    }
}