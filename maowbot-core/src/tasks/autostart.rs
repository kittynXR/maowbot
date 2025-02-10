// src/tasks/autostart.rs

use std::sync::Arc;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tracing::{info, warn, error};

use crate::Error;
use crate::repositories::postgres::bot_config::BotConfigRepository;
use crate::plugins::bot_api::BotApi;

/// JSON structure we store in `bot_config` under key = "autostart".
/// A simple shape: { "discord": ["MyDiscordBot"], "twitch-irc": ["MyIrcBot"] }
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutostartConfig(pub serde_json::Map<String, Value>);

impl AutostartConfig {
    pub fn new() -> Self {
        Self(serde_json::Map::new())
    }

    /// Given a platform name (like "discord") and an account name (like "myBot"),
    /// set autostart on/off. If on=true, add to the array; if on=false, remove from the array.
    pub fn set_platform_account(&mut self, platform: &str, account_name: &str, on: bool) {
        let entry = self.0.entry(platform.to_string()).or_insert_with(|| Value::Array(vec![]));
        let arr = entry.as_array_mut().unwrap();
        if on {
            // add if not present
            if !arr.iter().any(|v| v.as_str() == Some(account_name)) {
                arr.push(Value::String(account_name.to_string()));
            }
        } else {
            // remove if present
            arr.retain(|v| v.as_str() != Some(account_name));
        }
    }
}

/// Attempts to load the autostart config from DB (bot_config table),
/// parse it, then for each (platform -> list of accounts), tries to "start" them
/// by calling your BotApi's "start_runtime" or similar method.
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

    for (platform, arr_val) in autoconf.0.iter() {
        let arr = arr_val.as_array().cloned().unwrap_or_default();
        for account_name_val in arr {
            if let Some(account_name) = account_name_val.as_str() {
                // We'll call "start_platform" from the BotApi or from a new method we define.
                // For the TUI approach, let's define a new method in BotApi: `start_runtime(platform_str, account_str)`.
                // But if it doesn't exist, we can just rely on the TUI logic we wrote for "start" command,
                // or create an internal function. We'll demonstrate a direct call to "start" as if it existed.

                info!("Autostart: attempting to start platform='{}', account='{}'", platform, account_name);
                let res = bot_api.start_platform_runtime(platform, account_name).await;
                if let Err(e) = res {
                    error!("Autostart failed for platform='{}' account='{}': {:?}", platform, account_name, e);
                }
            }
        }
    }

    Ok(())
}
