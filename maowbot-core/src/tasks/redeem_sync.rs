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
use chrono::Utc;
use uuid::Uuid;

/// Finds a Helix reward in `list` whose title matches `wanted_title` (case-insensitive).
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

/// Returns true if `reward_id` is found in `list`.
fn is_in_list(list: &[CustomReward], reward_id: &str) -> bool {
    list.iter().any(|r| r.id == reward_id)
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
/// to build a Helix client. Returns None if not found or missing client_id.
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

/// Main routine to unify local DB redeems with Helix for a single account (token).
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

    // 2A) NEW: Import any Helix rewards not found in DB => create them as is_managed=false
    //           so they show up in the TUI as “web‑app managed”.
    for hr in &all_list {
        let existing = db_redeems.iter().find(|rd| rd.reward_id == hr.id);
        if existing.is_none() {
            // Insert new DB row with is_managed=false, plugin_name=None, etc.
            let now = Utc::now();
            let new_rd = Redeem {
                redeem_id: Uuid::new_v4(),
                platform: "twitch-eventsub".to_string(),
                reward_id: hr.id.clone(),
                reward_name: hr.title.clone(),
                cost: hr.cost as i32,
                is_active: hr.is_enabled,
                dynamic_pricing: false,
                created_at: now,
                updated_at: now,
                active_offline: false,
                is_managed: false, // Because the broadcaster created it in the dashboard
                plugin_name: None,
                command_name: None,
            };
            if let Err(e) = redeem_service.redeem_repo.create_redeem(&new_rd).await {
                error!("Failed to import 'web-app' reward='{}' => {e}", hr.title);
            } else {
                debug!("Imported new web-app managed reward='{}', id={}", hr.title, hr.id);
            }
        }
    }

    // re‑fetch after import
    let db_redeems = redeem_service.list_redeems("twitch-eventsub").await?;

    // 3) If any Helix reward is found, unify `is_managed` for matching rows
    for hr in &all_list {
        let is_m = manageable_list.iter().any(|mr| mr.id == hr.id);
        if let Some(dbrow) = db_redeems.iter().find(|d| d.reward_id == hr.id) {
            if dbrow.is_managed != is_m {
                let mut to_update = dbrow.clone();
                to_update.is_managed = is_m;
                to_update.updated_at = Utc::now();
                // Special case: if plugin_name="builtin", keep is_managed = true
                if dbrow.plugin_name.as_deref() == Some("builtin") {
                    to_update.is_managed = true;
                }
                if let Err(e) = redeem_service.redeem_repo.update_redeem(&to_update).await {
                    error!("Error updating is_managed for '{}' => {e}", dbrow.reward_name);
                } else {
                    debug!("Set is_managed={} for '{}'", to_update.is_managed, dbrow.reward_name);
                }
            }
        }
    }

    // 4) For each DB redeem, if it is “builtin” or `is_managed=true`, ensure Helix has it
    for dr in &db_redeems {
        let is_builtin = dr.plugin_name.as_deref() == Some("builtin");
        let effective_managed = dr.is_managed || is_builtin;

        // If builtin + reward_id is empty => unify by name or create
        if is_builtin && dr.reward_id.trim().is_empty() {
            if let Some(existing_id) = find_reward_id_by_title_ignorecase(&all_list, &dr.reward_name) {
                info!(
                    "Builtin redeem '{}' matches existing Helix reward_id='{}'. Linking them.",
                    dr.reward_name, existing_id
                );
                let mut updated = dr.clone();
                updated.reward_id = existing_id;
                updated.is_managed = true;
                updated.updated_at = Utc::now();
                if let Err(e) = redeem_service.redeem_repo.update_redeem(&updated).await {
                    error!("Error linking builtin redeem => {e}");
                }
                continue;
            } else {
                // create new Helix reward
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
                        updated.is_managed = true;
                        updated.updated_at = Utc::now();
                        if let Err(e) = redeem_service.redeem_repo.update_redeem(&updated).await {
                            error!("Error storing new Helix ID => {e}");
                        } else {
                            debug!("Builtin '{}' => assigned new reward_id='{}'", updated.reward_name, updated.reward_id);
                        }
                    }
                    Err(e) => {
                        let e_str = format!("{e}");
                        if e_str.contains("CREATE_CUSTOM_REWARD_DUPLICATE_REWARD") {
                            warn!(
                                "Unable to create Helix reward for builtin '{}' => Duplicate. Attempting fallback unify...",
                                dr.reward_name
                            );
                            if let Ok(refreshed_all) = client.get_custom_rewards(&broadcaster_id, None, false).await {
                                if let Some(dup_id) = find_reward_id_by_title_ignorecase(&refreshed_all, &dr.reward_name) {
                                    let mut updated = dr.clone();
                                    updated.reward_id = dup_id;
                                    updated.is_managed = true;
                                    updated.updated_at = Utc::now();
                                    if let Err(e2) = redeem_service.redeem_repo.update_redeem(&updated).await {
                                        error!("Error fallback linking => {e2}");
                                    }
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

        // If is_managed and missing from Helix => create
        if effective_managed && dr.reward_id.trim().is_empty() {
            // we do the same create logic
            info!("Managed redeem '{}' has empty reward_id => create in Helix", dr.reward_name);
            let body = CustomRewardBody {
                title: Some(dr.reward_name.clone()),
                cost: Some(dr.cost as u64),
                is_enabled: Some(dr.is_active),
                ..Default::default()
            };
            if let Err(e) = try_create_new_reward(&broadcaster_id, &body, dr, redeem_service, client).await {
                warn!("Error from try_create_new_reward => {e}");
            }
            continue;
        }
    }

    // 5) re-fetch after any creation
    let updated_helix = client.get_custom_rewards(&broadcaster_id, None, false).await.unwrap_or(all_list);

    // 6) cost/is_active sync for all “managed or builtin” redeems
    for dr in &db_redeems {
        let is_builtin = dr.plugin_name.as_deref() == Some("builtin");
        let effective_managed = dr.is_managed || is_builtin;

        if !effective_managed {
            continue;
        }
        let rid = dr.reward_id.trim();
        if rid.is_empty() {
            continue;
        }

        if let Some(hrew) = updated_helix.iter().find(|r| r.id == rid) {
            // if stream offline => disable if not offline-allowed
            if !is_stream_online && !dr.active_offline && dr.is_active {
                info!("Stream offline => disabling redeem='{}'", dr.reward_name);
                let patch_body = CustomRewardBody { is_enabled: Some(false), ..Default::default() };
                if let Err(e) = client.update_custom_reward(&broadcaster_id, rid, &patch_body).await {
                    error!("update_custom_reward => {e}");
                }
                let mut upd = dr.clone();
                upd.is_active = false;
                upd.updated_at = Utc::now();
                if let Err(e) = redeem_service.redeem_repo.update_redeem(&upd).await {
                    error!("Error updating DB => {e}");
                }
                continue;
            }

            // check cost or is_active mismatch
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
                if let Err(e) = client.update_custom_reward(&broadcaster_id, rid, &patch_body).await {
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

// Helper for “create a new Helix reward, then update DB row’s reward_id”.
async fn try_create_new_reward(
    broadcaster_id: &str,
    body: &CustomRewardBody,
    dr: &Redeem,
    redeem_service: &RedeemService,
    client: &TwitchHelixClient
) -> Result<(), Error> {
    match client.create_custom_reward(broadcaster_id, body).await {
        Ok(created) => {
            let mut updated = dr.clone();
            updated.reward_id = created.id;
            if updated.plugin_name.as_deref() == Some("builtin") {
                updated.is_managed = true;
            }
            updated.updated_at = Utc::now();
            redeem_service.redeem_repo.update_redeem(&updated).await?;
            debug!("Redeem '{}' => assigned new reward_id='{}'", updated.reward_name, updated.reward_id);
        }
        Err(e) => {
            return Err(e);
        }
    }
    Ok(())
}
