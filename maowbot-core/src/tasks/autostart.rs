use std::sync::Arc;
use tracing::{info, error, warn};
use maowbot_common::traits::api::BotApi;
use crate::Error;
use crate::repositories::postgres::autostart::AutostartRepository;

/// Called at startup to read autostart entries from the database and start each
/// enabled platform/account. If `platform == "twitch-irc"`, we *also* auto-join channels
/// corresponding to all other Twitch-IRC accounts.
pub async fn run_autostart(
    autostart_repo: &dyn AutostartRepository,
    bot_api: Arc<dyn BotApi>,
) -> Result<(), Error> {
    // Get all enabled autostart entries
    let entries = autostart_repo.get_enabled_entries().await?;
    
    if entries.is_empty() {
        info!("No autostart entries found. Skipping autostart.");
        return Ok(());
    }
    
    info!("Found {} autostart entries to process", entries.len());

    for entry in entries {
        info!("Autostart: attempting to start platform='{}', account='{}'", entry.platform, entry.account_name);
        if let Err(e) = bot_api.start_platform_runtime(&entry.platform, &entry.account_name).await {
            error!(
                "Autostart failed for platform='{}', account='{}': {:?}",
                entry.platform, entry.account_name, e
            );
            continue;
        }
        
        // If it's twitch-irc, do the "auto join all other accounts" approach
        if entry.platform.eq_ignore_ascii_case("twitch-irc") {
            match auto_join_twitch_other_accounts(bot_api.clone(), &entry.account_name).await {
                Ok(_) => info!("Autostart: joined all other Twitch-IRC channels for '{}'", entry.account_name),
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
    // find the one that matches "started_account"
    let me_opt = all_creds
        .iter()
        .find(|c| c.user_name.eq_ignore_ascii_case(started_account));

    if let Some(me) = me_opt {
        // for each other, do "join #theirUserName"
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