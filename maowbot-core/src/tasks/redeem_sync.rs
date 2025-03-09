use tracing::{info, error, warn, debug};
use crate::Error;
use crate::services::twitch::redeem_service::RedeemService;
use crate::platforms::manager::PlatformManager;
use crate::services::user_service::UserService;
use crate::platforms::twitch::client::TwitchHelixClient;
use crate::platforms::twitch::requests::channel_points::{CustomRewardBody, CustomReward};
use crate::models::Redeem;
use crate::repositories::postgres::bot_config::BotConfigRepository;
use serde::Deserialize;

/// We re-use the shape below to parse the `autostart` config from your bot_config.
#[derive(Debug, Deserialize)]
pub struct AutostartConfig {
    pub accounts: Vec<(String, String)>, // e.g. [ ("twitch-irc", "Kittyn"), ("twitch-irc", "Synapsycat") ]
}

/// The main function that enumerates all the Twitch accounts from autostart
/// and does a Helix-based sync for each.
pub async fn sync_channel_redeems(
    redeem_service: &RedeemService,
    platform_manager: &PlatformManager,
    user_service: &UserService,
    bot_config_repo: &dyn BotConfigRepository,
    is_stream_online: bool,
) -> Result<(), Error> {
    info!("Running channel redeem sync => is_stream_online={}", is_stream_online);

    // 1) Load and parse the autostart config from bot_config
    let val_opt = bot_config_repo.get_value("autostart").await?;
    if val_opt.is_none() {
        warn!("No autostart config found => no accounts to sync. Skipping redeem sync.");
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

    // 2) For each (platform, account) in autostart => if it's Twitch or twitch-irc, do Helix sync logic.
    //    We'll gather the DB's "twitch-eventsub" redeems once; that’s how we store them.
    let all_redeems = redeem_service.list_redeems("twitch-eventsub").await?;
    info!("sync_channel_redeems: Found {} DB redeems on 'twitch-eventsub'.", all_redeems.len());

    for (pf, acct) in &autoconf.accounts {
        if pf.eq_ignore_ascii_case("twitch") || pf.eq_ignore_ascii_case("twitch-irc") {
            info!("sync_channel_redeems: Checking Helix credentials => platform='{}', account='{}'", pf, acct);

            let maybe_client = get_helix_client_for_account(platform_manager, user_service, acct).await?;
            if let Some(client) = maybe_client {
                info!("Syncing redeems for account='{}' => Helix calls", acct);
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
    // 1) figure out the broadcaster_id from validate()
    let broadcaster_id = match client.validate_token().await {
        Ok(Some(validate_resp)) => validate_resp.user_id,
        Ok(None) => {
            // means token is invalid or we got no user_id
            return Ok(());
        }
        Err(e) => {
            error!("Error calling /validate => {:?}", e);
            return Ok(());
        }
    };

    // 2) Query Helix for all custom rewards
    let helix_rewards = client.get_custom_rewards(&broadcaster_id, None, false).await?;
    info!(
        "run_sync_for_one_account: Helix returned {} custom rewards for broadcaster_id={}",
        helix_rewards.len(),
        broadcaster_id
    );

    // 3) Cross-check: any new rewards in Helix not in DB => insert as is_managed=false.
    for hr in &helix_rewards {
        let db_match = db_redeems.iter().find(|d| d.reward_id == hr.id);
        if db_match.is_none() {
            info!("Found new reward '{}' on Twitch that is not in DB => is_managed=false", hr.title);
            let new_rd = crate::models::Redeem {
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

    // 4) If DB says is_managed=true but Helix does NOT have it => try create or fallback
    for dr in db_redeems {
        if dr.is_managed {
            let found_helix = helix_rewards.iter().any(|hr| hr.id == dr.reward_id);
            if !found_helix {
                info!(
                    "Managed redeem '{}' not found in Helix => attempt create_custom_reward or fallback",
                    dr.reward_name
                );
                let body = CustomRewardBody {
                    title: Some(dr.reward_name.clone()),
                    cost: Some(dr.cost as u64),
                    is_enabled: Some(dr.is_active),
                    ..Default::default()
                };

                // Attempt creation
                match client.create_custom_reward(&broadcaster_id, &body).await {
                    Ok(created) => {
                        info!("Created reward in Helix => ID={}", created.id);
                        // Update DB with the new Helix ID
                        let mut updated = dr.clone();
                        updated.reward_id = created.id;
                        updated.updated_at = chrono::Utc::now();
                        redeem_service.redeem_repo.update_redeem(&updated).await?;
                    }
                    Err(e) => {
                        let e_str = format!("{e}");
                        if e_str.contains("CREATE_CUSTOM_REWARD_DUPLICATE_REWARD") {
                            warn!("Duplicate reward => searching for the matching Helix reward by title='{}'", dr.reward_name);
                            // Look again at Helix to see if the same title is present
                            if let Ok(refreshed) = client.get_custom_rewards(&broadcaster_id, None, false).await {
                                if let Some(existing) = find_reward_by_title(&refreshed, &dr.reward_name) {
                                    // We adopt that Helix ID
                                    let mut updated = dr.clone();
                                    updated.reward_id = existing.id.clone();
                                    updated.updated_at = chrono::Utc::now();
                                    redeem_service.redeem_repo.update_redeem(&updated).await?;
                                    info!("Updated DB redeem '{}' => now reward_id='{}'", dr.reward_name, existing.id);
                                } else {
                                    warn!(
                                        "No matching reward found in Helix for title='{}' => cannot fix duplicate error.",
                                        dr.reward_name
                                    );
                                }
                            }
                        } else {
                            error!("create_custom_reward => {e}");
                        }
                    }
                }
            }
        }
    }

    // 4.5) **NEW** For all is_managed redeems that we DO find in Helix, check for changes
    //      in cost, is_active, etc., and patch them if they differ.
    for dr in db_redeems {
        // Is it a bot-managed redeem that also exists in Helix?
        if dr.is_managed {
            if let Some(helix_rd) = helix_rewards.iter().find(|hr| hr.id == dr.reward_id) {
                // Compare cost or is_active
                let cost_mismatch = (dr.cost as u64) != helix_rd.cost;
                let active_mismatch = dr.is_active != helix_rd.is_enabled;
                if cost_mismatch || active_mismatch {
                    // We'll do a partial update to Helix
                    let patch_body = CustomRewardBody {
                        cost: if cost_mismatch { Some(dr.cost as u64) } else { None },
                        is_enabled: if active_mismatch { Some(dr.is_active) } else { None },
                        ..Default::default()
                    };
                    info!(
                        "Updating Helix reward='{}' ID={} => cost={}, is_enabled={}",
                        dr.reward_name, dr.reward_id, dr.cost, dr.is_active
                    );
                    if let Err(e) = client.update_custom_reward(&broadcaster_id, &dr.reward_id, &patch_body).await {
                        error!("update_custom_reward => {e}");
                    }
                }
            }
        }
    }

    // 5) If stream is offline => disable any is_managed redeems that do not allow offline usage
    if !is_stream_online {
        for dr in db_redeems {
            if dr.is_managed && !dr.active_offline && dr.is_active {
                info!("Stream offline => disabling redeem='{}' in Helix + DB", dr.reward_name);
                let patch_body = CustomRewardBody {
                    is_enabled: Some(false),
                    ..Default::default()
                };
                // Helix update
                let _ = client.update_custom_reward(&broadcaster_id, &dr.reward_id, &patch_body).await;

                // DB update
                let mut updated = dr.clone();
                updated.is_active = false;
                updated.updated_at = chrono::Utc::now();
                redeem_service.redeem_repo.update_redeem(&updated).await?;
            }
        }
    }

    Ok(())
}

/// Helper to find a reward by case-insensitive title.
fn find_reward_by_title<'a>(
    rewards: &'a [CustomReward],
    title: &str
) -> Option<&'a CustomReward> {
    let lowered_title = title.to_lowercase();
    rewards.iter().find(|r| r.title.to_lowercase() == lowered_title)
}

/// Helper: get Helix client for a given <account_name>, if we have a stored credential.
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
