// File: maowbot-core/src/tasks/autostart.rs

use std::sync::Arc;
use serde::{Deserialize, Serialize};
use tracing::{info, error, warn};
use maowbot_common::traits::api::BotApi;
use crate::Error;
use crate::repositories::postgres::bot_config::BotConfigRepository;

/// This struct now matches the TUI’s shape:
/// { "accounts": [ ["discord","cutecat_chat"], ["twitch-irc","myIrcAccount"] ] }
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutostartConfig {
    pub accounts: Vec<(String, String)>, // (platform, account)
}

impl AutostartConfig {
    pub fn new() -> Self {
        Self { accounts: vec![] }
    }

    /// Called by the TUI to set/unset a particular (platform, account).
    pub fn set_platform_account(&mut self, platform: &str, account_name: &str, on: bool) {
        if on {
            // add if not present
            if !self.accounts.iter().any(|(pf, acct)| pf == platform && acct == account_name) {
                self.accounts.push((platform.to_string(), account_name.to_string()));
            }
        } else {
            // remove if present
            self.accounts.retain(|(pf, acct)| !(pf == platform && acct == account_name));
        }
    }
}

/// Loads the `autostart` JSON from `bot_config`, parses it, and calls
/// `start_platform_runtime(platform, account)` for each item.
/// If any `platform` is "twitch-irc", also replicate the TUI’s logic to join
/// broadcaster and secondary channels (so it’s fully “connected”).
pub async fn run_autostart(
    bot_config_repo: &(dyn BotConfigRepository + Send + Sync),
    bot_api: Arc<dyn BotApi>,
) -> Result<(), Error> {
    let val_opt = bot_config_repo.get_value("autostart").await?;
    if val_opt.is_none() {
        info!("No autostart config found in bot_config. Skipping autostart.");
        return Ok(());
    }

    let val_str = val_opt.unwrap();
    let parsed: serde_json::Result<AutostartConfig> = serde_json::from_str(&val_str);
    let autoconf = match parsed {
        Ok(c) => c,
        Err(e) => {
            error!("Failed to parse 'autostart' JSON: {}", e);
            return Ok(()); // do not crash; just skip
        }
    };

    for (platform, account) in autoconf.accounts.iter() {
        info!(
            "Autostart: attempting to start platform='{}', account='{}'",
            platform, account
        );

        // Step 1: Start the runtime for (platform, account).
        let res = bot_api.start_platform_runtime(platform, account).await;
        if let Err(e) = res {
            error!(
                "Autostart failed for platform='{}' account='{}': {:?}",
                platform, account, e
            );
            continue;
        }

        // Step 2: If Twitch‑IRC, replicate the TUI’s channel-join logic.
        if platform.eq_ignore_ascii_case("twitch-irc") {
            match auto_join_twitch_channels(bot_config_repo, bot_api.clone(), account).await {
                Ok(_) => {
                    info!("Autostart: joined broadcaster/secondary channels for {}", account);
                }
                Err(e) => {
                    warn!(
                        "Autostart: platform='twitch-irc' channel-join partial failure => {:?}",
                        e
                    );
                }
            }
        }
    }

    Ok(())
}

/// For a Twitch‑IRC account, we mimic the TUI behavior:
///  1) Retrieve `ttv_broadcaster_channel` from bot_config (if any).
///  2) Retrieve `ttv_secondary_account` from bot_config (if any).
///  3) For each channel, auto‑prepend '#' if missing, then call `join_twitch_irc_channel`.
/// Note: This does **not** track any local TUI state; it just ensures the same channels are joined.
async fn auto_join_twitch_channels(
    bot_config_repo: &(dyn BotConfigRepository + Send + Sync),
    bot_api: Arc<dyn BotApi>,
    account: &str,
) -> Result<(), Error> {
    let broadcaster = bot_config_repo.get_value("ttv_broadcaster_channel").await?;
    let secondary  = bot_config_repo.get_value("ttv_secondary_account").await?;

    // broadcaster
    if let Some(broadcaster_name) = broadcaster {
        let trimmed = broadcaster_name.trim();
        if !trimmed.is_empty() {
            let chan = if trimmed.starts_with('#') {
                trimmed.to_string()
            } else {
                format!("#{}", trimmed)
            };
            info!("auto_join_twitch_channels: joining broadcaster channel '{}'", chan);
            if let Err(e) = bot_api.join_twitch_irc_channel(account, &chan).await {
                warn!("Failed to join broadcaster channel '{}' => {:?}", chan, e);
            }
        }
    }

    // secondary
    if let Some(sec) = secondary {
        let trimmed = sec.trim();
        if !trimmed.is_empty() {
            // In the TUI, 'ttv_secondary_account' is often a *user account*,
            // but for joining channels we treat it the same: #someuser
            let chan = if trimmed.starts_with('#') {
                trimmed.to_string()
            } else {
                format!("#{}", trimmed)
            };
            info!("auto_join_twitch_channels: joining secondary channel '{}'", chan);
            if let Err(e) = bot_api.join_twitch_irc_channel(account, &chan).await {
                warn!("Failed to join secondary channel '{}' => {:?}", chan, e);
            }
        }
    }

    Ok(())
}
