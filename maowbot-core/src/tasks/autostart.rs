use std::sync::Arc;
use serde::{Deserialize, Serialize};
use tracing::{info, error, warn};
use maowbot_common::traits::api::BotApi;
use crate::Error;

/// Modified struct for storing which accounts to autostart:
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutostartConfig {
    pub accounts: Vec<(String, String)>, // (platform, account)
}

impl AutostartConfig {
    pub fn new() -> Self {
        Self { accounts: vec![] }
    }

    pub fn set_platform_account(&mut self, platform: &str, account_name: &str, on: bool) {
        if on {
            if !self.accounts.iter().any(|(pf, acct)| pf == platform && acct == account_name) {
                self.accounts.push((platform.to_string(), account_name.to_string()));
            }
        } else {
            self.accounts
                .retain(|(pf, acct)| !(pf == platform && acct == account_name));
        }
    }
}

/// Called at startup to read `autostart` from bot_config, parse it, then start each
/// `(platform, account)`. If `platform == "twitch-irc"`, we *also* auto-join channels
/// corresponding to all other Twitch-IRC accounts (the new approach).
pub async fn run_autostart(
    bot_config_repo: &dyn maowbot_common::traits::api::BotConfigApi,
    bot_api: Arc<dyn BotApi>,
) -> Result<(), Error> {
    let val_opt = bot_config_repo.get_bot_config_value("autostart").await?;
    let val_str = match val_opt {
        Some(s) => s,
        None => {
            info!("No autostart config found in bot_config. Skipping autostart.");
            return Ok(());
        }
    };

    let parsed: serde_json::Result<AutostartConfig> = serde_json::from_str(&val_str);
    let autoconf = match parsed {
        Ok(c) => c,
        Err(e) => {
            error!("Failed to parse 'autostart' JSON: {}", e);
            return Ok(()); // do not crash
        }
    };

    for (platform, account) in &autoconf.accounts {
        info!("Autostart: attempting to start platform='{}', account='{}'", platform, account);
        if let Err(e) = bot_api.start_platform_runtime(platform, account).await {
            error!(
                "Autostart failed for platform='{}', account='{}': {:?}",
                platform, account, e
            );
            continue;
        }
        // If it's twitch-irc, do the “auto join all other accounts” approach
        if platform.eq_ignore_ascii_case("twitch-irc") {
            match auto_join_twitch_other_accounts(bot_api.clone(), account).await {
                Ok(_) => info!("Autostart: joined all other Twitch-IRC channels for '{}'", account),
                Err(e) => warn!("Autostart: partial failure joining Twitch-IRC channels => {:?}", e),
            }
        }
    }

    Ok(())
}

/// After starting the given `twitch-irc` account, join the channels for every other
/// `twitch-irc` account in the credentials table.
async fn auto_join_twitch_other_accounts(
    bot_api: Arc<dyn BotApi>,
    started_account: &str
) -> Result<(), Error> {
    let all_creds = bot_api.list_credentials(Some(maowbot_common::models::platform::Platform::TwitchIRC)).await?;
    // find the one that matches “started_account”
    let me_opt = all_creds
        .iter()
        .find(|c| c.user_name.eq_ignore_ascii_case(started_account));

    if let Some(me) = me_opt {
        // for each other, do “join #theirUserName”
        for c in &all_creds {
            if c.user_name.eq_ignore_ascii_case(&me.user_name) {
                continue;
            }
            let chan = format!("#{}", c.user_name);
            if let Err(e) = bot_api.join_twitch_irc_channel(&me.user_name, &chan).await {
                warn!("(autostart) Could not join '{}': {:?}", chan, e);
            }
        }
    }

    Ok(())
}
