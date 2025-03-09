// maowbot-core/src/tasks/redeem_sync.rs

use tracing::{info, error, warn, debug, trace};
use crate::Error;
use crate::services::twitch::redeem_service::RedeemService;
use crate::platforms::manager::PlatformManager;
use crate::services::user_service::UserService;
use crate::platforms::twitch::client::TwitchHelixClient;
use crate::platforms::twitch::requests::channel_points::{
    CustomRewardBody,
    CustomReward,
};
use crate::models::Redeem;
use crate::repositories::postgres::bot_config::BotConfigRepository;

/// Finds a Helix reward in `list` whose title matches `wanted_title` (case-insensitive).
/// Returns that reward ID if found, else `None`.
fn find_reward_id_by_title_ignorecase(list: &[CustomReward], wanted_title: &str) -> Option<String> {
    let target = wanted_title.to_lowercase();
    list.iter().find_map(|r| {
        if r.title.to_lowercase() == target {
            Some(r.id.clone())
        } else {
            None
        }
    })
}

/// The main entry point for redeem sync.
pub async fn sync_channel_redeems(
    redeem_service: &RedeemService,
    platform_manager: &PlatformManager,
    user_service: &UserService,
    bot_config_repo: &dyn BotConfigRepository,
    is_stream_online: bool,
) -> Result<(), Error> {
    info!("Redeem sync started => is_stream_online={}", is_stream_online);

    // 1) DB Redeems on platform='twitch-eventsub'
    let all_redeems = redeem_service.list_redeems("twitch-eventsub").await?;
    info!(
        "[redeem_sync] Found {} DB redeems on platform='twitch-eventsub'.",
        all_redeems.len()
    );

    // 2) We'll try up to 2 accounts from bot_config
    let broadcaster_opt = bot_config_repo.get_value("ttv_broadcaster_channel").await?;
    let secondary_opt   = bot_config_repo.get_value("ttv_secondary_account").await?;
    let possible_users = [broadcaster_opt, secondary_opt];

    for maybe_acc in possible_users {
        if let Some(acc) = maybe_acc {
            let cleaned = acc.trim().trim_start_matches('#');
            if cleaned.is_empty() {
                continue;
            }
            info!("[redeem_sync] Attempting Helix sync => account='{}'", cleaned);
            if let Some(client) = get_helix_client_for_account(platform_manager, user_service, cleaned).await? {
                run_sync_for_one_account(&client, &all_redeems, redeem_service, is_stream_online).await?;
            } else {
                warn!("No Helix client available for '{}'", cleaned);
            }
        }
    }

    Ok(())
}

/// Attempts to find user by `global_username=account_name`, then fetch a Twitch credential
/// to build a Helix client. Returns None if not found.
pub async fn get_helix_client_for_account(
    platform_manager: &PlatformManager,
    user_service: &UserService,
    account_name: &str,
) -> Result<Option<TwitchHelixClient>, Error> {
    let user = match user_service.find_user_by_global_username(account_name).await {
        Ok(u) => u,
        Err(_) => {
            debug!("No local user found with global_username='{}'", account_name);
            return Ok(None);
        }
    };

    let maybe_cred = platform_manager
        .credentials_repo
        .get_credentials(&crate::models::Platform::Twitch, user.user_id)
        .await?;

    let cred = match maybe_cred {
        Some(c) => c,
        None => {
            debug!(
                "No Helix credential object found for user_id={} account='{}'",
                user.user_id, account_name
            );
            return Ok(None);
        }
    };

    if let Some(additional) = &cred.additional_data {
        if let Some(cid) = additional.get("client_id").and_then(|v| v.as_str()) {
            debug!(
                "Building HelixClient => user_id={} account='{}' client_id='{}'",
                user.user_id, account_name, cid
            );
            let client = TwitchHelixClient::new(&cred.primary_token, cid);
            return Ok(Some(client));
        }
    }
    debug!(
        "No valid 'client_id' in credential.additional_data => cannot build HelixClient (account='{}').",
        account_name
    );
    Ok(None)
}

/// Main routine to unify local DB redeems with Helix.
async fn run_sync_for_one_account(
    client: &TwitchHelixClient,
    db_redeems: &[Redeem],
    redeem_service: &RedeemService,
    is_stream_online: bool
) -> Result<(), Error> {
    // 1) Validate token => get broadcaster_id
    let broadcaster_id = match client.validate_token().await {
        Ok(Some(resp)) => {
            debug!("Validated => broadcaster_user_id='{}' login='{}'", resp.user_id, resp.login);
            resp.user_id
        },
        Ok(None) => {
            warn!("Helix token invalid/no user => skipping sync for this account.");
            return Ok(());
        },
        Err(e) => {
            error!("Error calling /validate => {e:?}");
            return Ok(());
        }
    };

    // 2) Get Helix rewards (both manageable & all)
    let manageable_list = client.get_custom_rewards(&broadcaster_id, None, true).await.unwrap_or_default();
    let all_list = client.get_custom_rewards(&broadcaster_id, None, false).await.unwrap_or_default();
    info!(
        "Helix returned {} total rewards, {} are manageable (broadcaster_id={})",
        all_list.len(),
        manageable_list.len(),
        broadcaster_id
    );

    // 3) If any DB row has that reward_id, unify is_managed
    for hr in &all_list {
        let is_m = manageable_list.iter().any(|mr| mr.id == hr.id);
        if let Some(dbrow) = db_redeems.iter().find(|d| d.reward_id == hr.id) {
            if dbrow.is_managed != is_m {
                let mut to_update = dbrow.clone();
                to_update.is_managed = is_m;
                to_update.updated_at = chrono::Utc::now();
                if let Err(e) = redeem_service.redeem_repo.update_redeem(&to_update).await {
                    error!("Error updating is_managed for '{}' => {e}", dbrow.reward_name);
                } else {
                    debug!("Set is_managed={} for '{}'", is_m, dbrow.reward_name);
                }
            }
        }
    }

    // 4) For each DB redeem, if `plugin_name='builtin'`, treat it as "bot-managed".
    //    If it lacks a valid reward_id, or not found in Helix, create or unify by name.
    for dr in db_redeems {
        let is_builtin = dr.plugin_name.as_deref() == Some("builtin");
        let effective_managed = dr.is_managed || is_builtin;

        // 4A) If builtin + empty reward_id => unify by name or create
        if is_builtin && dr.reward_id.trim().is_empty() {
            // search by name in all_list
            if let Some(existing_id) = find_reward_id_by_title_ignorecase(&all_list, &dr.reward_name) {
                info!(
                    "Builtin redeem '{}' matches existing Helix reward_id='{}'. Linking them.",
                    dr.reward_name, existing_id
                );
                let mut updated = dr.clone();
                updated.reward_id = existing_id.clone();
                // see if it's in manageable_list
                updated.is_managed = manageable_list.iter().any(|x| x.id == existing_id);
                updated.updated_at = chrono::Utc::now();
                if let Err(e) = redeem_service.redeem_repo.update_redeem(&updated).await {
                    error!("Error linking builtin redeem => {e}");
                }
                continue;
            } else {
                // create a new reward
                info!("No Helix reward for builtin '{}' => creating new custom reward", dr.reward_name);
                let body = CustomRewardBody {
                    title: Some(dr.reward_name.clone()),
                    cost: Some(dr.cost as u64),
                    is_enabled: Some(dr.is_active),
                    ..Default::default()
                };
                match client.create_custom_reward(&broadcaster_id, &body).await {
                    Ok(created) => {
                        let mut updated = dr.clone();
                        updated.reward_id = created.id;
                        updated.is_managed = true; // our builtins => we consider them "managed"
                        updated.updated_at = chrono::Utc::now();
                        if let Err(e) = redeem_service.redeem_repo.update_redeem(&updated).await {
                            error!("Error storing new Helix ID for builtin '{}' => {e}", dr.reward_name);
                        } else {
                            debug!("Builtin '{}' => assigned new reward_id='{}'", updated.reward_name, updated.reward_id);
                        }
                    }
                    Err(e) => {
                        let e_str = format!("{e}");
                        // <--- The fallback for DUPLICATE_REWARD:
                        if e_str.contains("CREATE_CUSTOM_REWARD_DUPLICATE_REWARD") {
                            warn!(
                                "Unable to create Helix reward for builtin '{}' => Duplicate. Attempting fallback unify...",
                                dr.reward_name
                            );
                            // Re-fetch Helix to see if a new item was just made or is being recognized
                            if let Ok(refreshed_all) = client.get_custom_rewards(&broadcaster_id, None, false).await {
                                if let Some(dup_id) = find_reward_id_by_title_ignorecase(&refreshed_all, &dr.reward_name) {
                                    info!(
                                        "Fallback unify: found Helix reward_id='{}' for builtin '{}'. Linking it.",
                                        dup_id, dr.reward_name
                                    );
                                    let mut updated = dr.clone();
                                    updated.reward_id = dup_id.clone();
                                    updated.is_managed = refreshed_all.iter().any(|r| r.id == dup_id
                                        && is_in_list(&manageable_list, &dup_id));
                                    updated.updated_at = chrono::Utc::now();
                                    if let Err(e2) = redeem_service.redeem_repo.update_redeem(&updated).await {
                                        error!("Error in fallback linking => {e2}");
                                    }
                                } else {
                                    warn!(
                                        "Fallback unify could not find reward matching '{}' in Helix. \
                                        Possibly a partial mismatch or leftover. If you keep seeing this, rename the reward.",
                                        dr.reward_name
                                    );
                                }
                            }
                        } else {
                            error!("Unable to create Helix reward for builtin '{}' => {e}", dr.reward_name);
                        }
                    }
                }
                continue;
            }
        }

        // 4B) If it's "is_managed" (and not builtin, or maybe builtin too),
        //     but Helix doesn't have that reward_id => create it.
        if effective_managed {
            let found_helix = all_list.iter().any(|r| r.id == dr.reward_id);
            if !found_helix && !is_builtin {
                info!("Managed redeem '{}' not found in Helix => create_custom_reward", dr.reward_name);
                let body = CustomRewardBody {
                    title: Some(dr.reward_name.clone()),
                    cost: Some(dr.cost as u64),
                    is_enabled: Some(dr.is_active),
                    ..Default::default()
                };
                match client.create_custom_reward(&broadcaster_id, &body).await {
                    Ok(created) => {
                        let mut updated = dr.clone();
                        updated.reward_id = created.id;
                        updated.is_managed = true;
                        updated.updated_at = chrono::Utc::now();
                        if let Err(e) = redeem_service.redeem_repo.update_redeem(&updated).await {
                            error!("Error updating DB with newly created Helix reward => {e}");
                        }
                    }
                    Err(e) => {
                        let e_str = format!("{e}");
                        if e_str.contains("CREATE_CUSTOM_REWARD_DUPLICATE_REWARD") {
                            warn!(
                                "Managed redeem '{}' => DUPLICATE_REWARD. Attempting fallback unify by name.",
                                dr.reward_name
                            );
                            if let Ok(refreshed_all) = client.get_custom_rewards(&broadcaster_id, None, false).await {
                                if let Some(dup_id) = find_reward_id_by_title_ignorecase(&refreshed_all, &dr.reward_name) {
                                    info!("Fallback unify => found ID='{}' for '{}'", dup_id, dr.reward_name);
                                    let mut updated = dr.clone();
                                    updated.reward_id = dup_id.clone();
                                    updated.is_managed = is_in_list(&manageable_list, &dup_id);
                                    updated.updated_at = chrono::Utc::now();
                                    if let Err(e2) = redeem_service.redeem_repo.update_redeem(&updated).await {
                                        error!("Error fallback-updating => {e2}");
                                    }
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

    // 5) re-fetch so we see newly created updates
    let updated_helix = client.get_custom_rewards(&broadcaster_id, None, false).await.unwrap_or(all_list);

    // 6) cost/is_active sync for all is_managed or builtin
    for dr in db_redeems {
        let is_builtin = dr.plugin_name.as_deref() == Some("builtin");
        let effective_managed = (dr.is_managed || is_builtin);

        if !effective_managed {
            continue;
        }
        if let Some(hrew) = updated_helix.iter().find(|r| r.id == dr.reward_id) {
            // offline => disable if not allowed
            if !is_stream_online && !dr.active_offline && dr.is_active {
                info!("Stream offline => disabling redeem='{}'", dr.reward_name);
                let patch_body = CustomRewardBody {
                    is_enabled: Some(false),
                    ..Default::default()
                };
                if let Err(e) = client.update_custom_reward(&broadcaster_id, &dr.reward_id, &patch_body).await {
                    error!("update_custom_reward => {e}");
                }
                let mut upd = dr.clone();
                upd.is_active = false;
                upd.updated_at = chrono::Utc::now();
                if let Err(e) = redeem_service.redeem_repo.update_redeem(&upd).await {
                    error!("Error updating DB => {e}");
                }
                continue;
            }

            let cost_mismatch = (dr.cost as u64) != hrew.cost;
            let active_mismatch = dr.is_active != hrew.is_enabled;
            if cost_mismatch || active_mismatch {
                debug!(
                    "Redeem '{}' mismatch => cost={} vs {}, is_active={} vs {}",
                    dr.reward_name, dr.cost, hrew.cost, dr.is_active, hrew.is_enabled
                );
                let patch_body = CustomRewardBody {
                    cost: if cost_mismatch { Some(dr.cost as u64) } else { None },
                    is_enabled: if active_mismatch { Some(dr.is_active) } else { None },
                    ..Default::default()
                };
                if let Err(e) = client.update_custom_reward(&broadcaster_id, &dr.reward_id, &patch_body).await {
                    error!("update_custom_reward => {e}");
                }
            }
        } else {
            trace!(
                "Managed redeem '{}' not found in updated_helix => possibly removed or re-labeled?",
                dr.reward_name
            );
        }
    }

    Ok(())
}

/// Returns true if `reward_id` is found in `list`.
fn is_in_list(list: &[CustomReward], reward_id: &str) -> bool {
    list.iter().any(|r| r.id == reward_id)
}
