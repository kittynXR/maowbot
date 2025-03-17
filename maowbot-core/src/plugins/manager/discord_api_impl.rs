use std::sync::Arc;
use async_trait::async_trait;
use maowbot_common::error::Error;
use maowbot_common::traits::api::DiscordApi;
use maowbot_common::models::discord::{DiscordGuildRecord, DiscordChannelRecord};
use twilight_cache_inmemory::InMemoryCache;
use twilight_model::id::marker::{GuildMarker};
use twilight_model::id::Id;

use crate::plugins::manager::PluginManager;

#[async_trait]
impl DiscordApi for PluginManager {
    async fn list_discord_guilds(
        &self,
        account_name: &str
    ) -> Result<Vec<DiscordGuildRecord>, Error> {
        // 1) Directly get the Arc<InMemoryCache> from the platform manager:
        let cache = self.platform_manager.get_discord_cache(account_name).await?;

        // 2) Iterate guilds from that cache
        let mut out = Vec::new();
        for guild_ref in cache.iter().guilds() {
            let guild_id = guild_ref.key();
            let data = guild_ref.value();
            let name = data.name().to_string();

            out.push(DiscordGuildRecord {
                account_name: account_name.to_string(),
                guild_id: guild_id.to_string(),
                guild_name: name,
                is_active: false,
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
            });
        }
        Ok(out)
    }

    async fn list_discord_channels(
        &self,
        account_name: &str,
        guild_id_str: &str
    ) -> Result<Vec<DiscordChannelRecord>, Error> {
        let cache = self.platform_manager.get_discord_cache(account_name).await?;

        let guild_id_num = guild_id_str.parse::<u64>()
            .map_err(|_| Error::Platform(format!("Guild ID '{guild_id_str}' not numeric")))?;
        let guild_id = Id::<GuildMarker>::new(guild_id_num);

        let mut out = Vec::new();
        for chan_ref in cache.iter().channels() {
            if chan_ref.value().guild_id == Some(guild_id) {
                let channel_id = chan_ref.key();
                let ch_data = chan_ref.value();
                let channel_name = ch_data.name.clone().unwrap_or_else(|| channel_id.to_string());

                out.push(DiscordChannelRecord {
                    account_name: account_name.to_string(),
                    guild_id: guild_id_str.to_string(),
                    channel_id: channel_id.to_string(),
                    channel_name,
                    is_active: false,
                    created_at: chrono::Utc::now(),
                    updated_at: chrono::Utc::now(),
                });
            }
        }
        Ok(out)
    }
}
