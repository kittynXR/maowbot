// maowbot-core/src/plugins/manager/discord_api_impl.rs
//
// This file implements Discord-specific API calls for listing known guilds
// (servers) and channels stored in our database, as well as setting the
// “active server” for a given Discord account. We rely on the repository
// to store the actual data (discord guilds, channels, etc.).
//
// We define a new trait DiscordApi, and an implementation DiscordApiImpl.
//
// The TUI can call these methods to get real data instead of returning dummy lists.

use std::sync::Arc;
use async_trait::async_trait;
use maowbot_common::error::Error;
use maowbot_common::models::discord::{DiscordAccountRecord, DiscordChannelRecord, DiscordGuildRecord};
use maowbot_common::traits::api::DiscordApi;
use maowbot_common::traits::repository_traits::DiscordRepository;
use crate::plugins::manager::PluginManager;

/// A trait for Discord-specific operations that your TUI or other plugins might call.
/// For example, listing servers or channels from the DB, setting an active server, etc.


/// A concrete implementation using our `DiscordRepository`.
pub struct DiscordApiImpl {
    pub discord_repo: Arc<dyn DiscordRepository + Send + Sync>,
}

impl DiscordApiImpl {
    pub fn new(discord_repo: Arc<dyn DiscordRepository + Send + Sync>) -> Self {
        Self { discord_repo }
    }
}

#[async_trait]
impl DiscordApi for PluginManager {
    async fn list_discord_guilds(&self, account_name: &str) -> Result<Vec<DiscordGuildRecord>, Error> {
        self.discord_repo.list_guilds_for_account(account_name).await
    }

    async fn list_discord_channels(
        &self,
        account_name: &str,
        guild_id: &str
    ) -> Result<Vec<DiscordChannelRecord>, Error> {
        self.discord_repo.list_channels_for_guild(account_name, guild_id).await
    }

    async fn set_discord_active_server(
        &self,
        account_name: &str,
        guild_id: &str
    ) -> Result<(), Error> {
        self.discord_repo.set_active_server(account_name, guild_id).await
    }

    async fn get_discord_active_server(&self, account_name: &str) -> Result<Option<String>, Error> {
        self.discord_repo.get_active_server(account_name).await
    }

    async fn list_discord_accounts(&self) -> Result<Vec<DiscordAccountRecord>, Error> {
        // delegate to the repository
        self.discord_repo.list_accounts().await
    }

    async fn set_discord_active_account(&self, account_name: &str) -> Result<(), Error> {
        // delegate to the repository
        self.discord_repo.set_active_account(account_name).await
    }

    async fn get_discord_active_account(&self) -> Result<Option<String>, Error> {
        // read from the repository
        self.discord_repo.get_active_account().await
    }

    async fn set_discord_active_channel(
        &self,
        account_name: &str,
        guild_id: &str,
        channel_id: &str
    ) -> Result<(), Error> {
        self.discord_repo.set_active_channel(account_name, guild_id, channel_id).await
    }

    async fn get_discord_active_channel(
        &self,
        account_name: &str,
        guild_id: &str
    ) -> Result<Option<String>, Error> {
        self.discord_repo.get_active_channel(account_name, guild_id).await
    }
}
