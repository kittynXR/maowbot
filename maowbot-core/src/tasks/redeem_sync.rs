// File: maowbot-core/src/tasks/redeem_sync.rs

use tracing::{info, error, warn};
use crate::Error;
use crate::services::twitch::redeem_service::RedeemService;
use crate::platforms::manager::PlatformManager;
use crate::services::user_service::UserService;
use crate::platforms::twitch::client::TwitchHelixClient;
use crate::platforms::twitch::requests::channel_points::CustomRewardBody;
use crate::models::Redeem;
use crate::repositories::postgres::bot_config::BotConfigRepository;  // we’ll need to read autostart
use serde::Deserialize;

/// This is basically the same shape as your `AutostartConfig`.
#[derive(Debug, Deserialize)]
pub struct AutostartConfig {
    pub accounts: Vec<(String, String)>, // e.g. [ ("twitch-irc", "MyBroadcaster"), ... ]
}

/// The main function that enumerates all the Twitch accounts from autostart
/// and does the Helix-based sync for each.
///
/// We pass an additional `bot_config_repo` param so we can read `autostart`.
pub async fn sync_channel_redeems(
    redeem_service: &RedeemService,
    platform_manager: &PlatformManager,
    user_service: &UserService,
    bot_config_repo: &dyn BotConfigRepository,
    is_stream_online: bool,
) -> Result<(), Error> {
    info!("Running channel redeem sync => is_stream_online={}", is_stream_online);

    // 1) Load and parse the autostart config.
    //    If missing or invalid, we bail (or skip).
    let val_opt = bot_config_repo.get_value("autostart").await?;
    if val_opt.is_none() {
        warn!("No autostart config found => no accounts to sync. Skipping.");
        return Ok(());
    }

    let val_str = val_opt.unwrap();
    let parsed: serde_json::Result<AutostartConfig> = serde_json::from_str(&val_str);
    let autoconf = match parsed {
        Ok(c) => c,
        Err(e) => {
            error!("Failed to parse 'autostart' JSON: {}", e);
            return Ok(()); // just skip, do not crash
        }
    };

    // 2) For each (platform, account) in autostart => if it's Twitch, do Helix sync logic.
    let all_redeems = redeem_service.list_redeems("twitch-eventsub").await?;
    // ^ If you want to store multiple “platforms” in DB, you might do something else.
    //   For now, we assume only "twitch-eventsub" is relevant.

    for (pf, acct) in &autoconf.accounts {
        // We consider both "twitch" or "twitch-irc" as valid triggers for Helix sync
        // (depending on how you store your credentials).
        if pf.eq_ignore_ascii_case("twitch") || pf.eq_ignore_ascii_case("twitch-irc") {
            info!("sync_channel_redeems: Checking for Helix client => platform='{}', account='{}'", pf, acct);

            let maybe_client = get_helix_client_for_account(platform_manager, user_service, acct).await?;
            if let Some(client) = maybe_client {
                // If we successfully built a HelixClient, do the actual sync steps
                info!("Syncing redeems for account='{}' (platform='{}') => Helix", acct, pf);
                run_sync_for_one_account(&client, &all_redeems, redeem_service, is_stream_online).await?;
            } else {
                warn!("No Helix credential found for account='{}' => skipping sync", acct);
            }
        }
    }

    Ok(())
}

/// Actually runs the logic that compares DB vs. Helix for a single channel (one Helix client).
async fn run_sync_for_one_account(
    client: &TwitchHelixClient,
    db_redeems: &Vec<Redeem>,
    redeem_service: &RedeemService,
    is_stream_online: bool
) -> Result<(), Error> {
    // 1) Figure out the broadcaster_id from the client, if needed:
    //    Because we called `TwitchHelixClient::new(bearer_token, client_id)`,
    //    you can do a /validate call to get the user_id if you want:
    //    For a simpler approach, if you stored the user_id in your credential,
    //    you can also get it from there.
    //    For demonstration, let’s do a quick validate:

    let broadcaster_id = match client.validate_token().await {
        Ok(Some(validate_resp)) => validate_resp.user_id,
        Ok(None) => {
            // means token is invalid or we got no user_id
            return Ok(());
        }
        Err(e) => {
            error!("Error calling /validate => {:?}", e);
            return Ok(()); // skip
        }
    };

    // 2) Query Helix for all custom rewards
    let helix_rewards = client.get_custom_rewards(&broadcaster_id, None, false).await?;
    info!("run_sync_for_one_account: Helix returned {} custom rewards for broadcaster_id={}",
          helix_rewards.len(), broadcaster_id);

    // 3) Cross-check
    //    (Same logic as your existing single-broadcaster code)
    for hr in &helix_rewards {
        let db_match = db_redeems.iter().find(|d| d.reward_id == hr.id);
        if db_match.is_none() {
            info!("Found new reward '{}' on Twitch not in DB => is_managed=false", hr.title);
            let new_rd = Redeem {
                redeem_id: uuid::Uuid::new_v4(),
                platform: "twitch-eventsub".into(),
                reward_id: hr.id.clone(),
                reward_name: hr.title.clone(),
                cost: hr.cost as i32,
                is_active: hr.is_enabled,
                dynamic_pricing: false,
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
                active_offline: false,
                is_managed: false,
                plugin_name: None,
                command_name: None,
            };
            redeem_service.redeem_repo.create_redeem(&new_rd).await?;
        }
    }

    // 4) If DB has is_managed=true but Helix is missing it => re-create or disable
    for dr in db_redeems {
        if dr.is_managed {
            let found_helix = helix_rewards.iter().any(|hr| hr.id == dr.reward_id);
            if !found_helix {
                info!("Managed redeem '{}' not found in Helix => re-create or disable", dr.reward_name);
                let body = CustomRewardBody {
                    title: Some(dr.reward_name.clone()),
                    cost: Some(dr.cost as u64),
                    is_enabled: Some(dr.is_active),
                    ..Default::default()
                };
                // attempt to create
                let _ = client.create_custom_reward(&broadcaster_id, &body).await;
            }
        }
    }

    // 5) If stream offline => disable built-in redeems that have !active_offline
    if !is_stream_online {
        for dr in db_redeems {
            if dr.is_managed && !dr.active_offline && dr.is_active {
                info!("Stream offline => disabling redeem='{}' in Helix + DB", dr.reward_name);
                let patch_body = CustomRewardBody {
                    is_enabled: Some(false),
                    ..Default::default()
                };
                let _ = client.update_custom_reward(&broadcaster_id, &dr.reward_id, &patch_body).await;

                let mut updated = dr.clone();
                updated.is_active = false;
                updated.updated_at = chrono::Utc::now();
                redeem_service.redeem_repo.update_redeem(&updated).await?;
            }
        }
    }

    Ok(())
}

/// The same helper you had, with no mention of “YourChannelNameHere”
pub async fn get_helix_client_for_account(
    platform_manager: &PlatformManager,
    user_service: &UserService,
    account_name: &str,
) -> Result<Option<TwitchHelixClient>, Error> {
    let user = match user_service.find_user_by_global_username(account_name).await {
        Ok(u) => u,
        Err(_) => return Ok(None),
    };
    let maybe_cred = platform_manager.credentials_repo
        .get_credentials(&crate::models::Platform::Twitch, user.user_id)
        .await?;
    let cred = match maybe_cred {
        Some(c) => c,
        None => return Ok(None),
    };
    if let Some(additional) = &cred.additional_data {
        if let Some(cid) = additional.get("client_id").and_then(|v| v.as_str()) {
            let client = TwitchHelixClient::new(&cred.primary_token, cid);
            return Ok(Some(client));
        }
    }
    Ok(None)
}
