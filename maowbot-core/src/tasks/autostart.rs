// File: src/tasks/autostart.rs

use std::sync::Arc;
use serde::{Deserialize, Serialize};
use tracing::{info, error};

use crate::Error;
use crate::repositories::postgres::bot_config::BotConfigRepository;
use crate::plugins::bot_api::BotApi;

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
            if !self
                .accounts
                .iter()
                .any(|(pf, acct)| pf == platform && acct == account_name)
            {
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
        let res = bot_api.start_platform_runtime(platform, account).await;
        if let Err(e) = res {
            error!(
                "Autostart failed for platform='{}' account='{}': {:?}",
                platform, account, e
            );
        }
    }

    Ok(())
}
