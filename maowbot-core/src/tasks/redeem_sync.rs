// File: maowbot-core/src/tasks/redeem_sync.rs

use tracing::{info, error, warn, debug, trace};
use crate::Error;
use crate::services::twitch::redeem_service::RedeemService;
use crate::platforms::manager::PlatformManager;
use crate::services::user_service::UserService;
use crate::platforms::twitch::client::TwitchHelixClient;
use crate::platforms::twitch::requests::channel_points::{CustomRewardBody, CustomReward};
use crate::repositories::postgres::bot_config::BotConfigRepository;
use chrono::Utc;
use uuid::Uuid;
use maowbot_common::models::platform::Platform;
use maowbot_common::models::Redeem;
use maowbot_common::traits::repository_traits::{CredentialsRepository, RedeemRepository};

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

fn is_in_list(list: &[CustomReward], reward_id: &str) -> bool {
    list.iter().any(|r| r.id == reward_id)
}

/// The main function to sync local DB redeems to Twitch Helix **and** import
/// any previously unknown Twitch rewards into our DB.
///
/// In other words, it’s now a **two-way** sync:
/// 1) Fetch all Helix custom rewards, create any missing DB rows.
/// 2) For each DB redeem that is “managed,” ensure it’s up-to-date on Twitch
///    (cost, active, etc.), possibly creating or patching it on Helix if needed.
pub async fn sync_channel_redeems(
    redeem_service: &RedeemService,
    _platform_manager: &PlatformManager,
    _user_service: &UserService,
    _bot_config_repo: &dyn BotConfigRepository,
    is_stream_online: bool,
) -> Result<(), Error> {
    info!("Redeem sync started => is_stream_online={}", is_stream_online);

    // We now do it in two phases:
    // PHASE A: For each Helix reward that does NOT exist in DB, create a local row.
    // PHASE B: For each local DB redeem, attempt to create/patch in Helix if `is_managed`.

    // A) Gather all DB redeems:
    let db_redeems = redeem_service.list_redeems("twitch-eventsub").await?;
    info!(
        "[redeem_sync] Found {} DB redeems on platform='twitch-eventsub'.",
        db_redeems.len()
    );

    // B) For *each* Helix credential (or specifically for the broadcaster or chosen credential):
    //    Actually, we only need to do it for each *unique channel* the bot is “active” for.
    //
    // In many setups, you might have multiple Twitch Helix credentials. The simplest approach
    // is to do the “broadcaster” credential only, or do it for each credential that is_broadcaster=true.
    // Below, we just demonstrate for “the broadcaster”:

    // 1) Find the broadcaster’s Helix credential.
    let creds_repo = &redeem_service.credentials_repo;
    let broadcaster_cred_opt = creds_repo.get_broadcaster_credential(&Platform::Twitch).await?;
    if broadcaster_cred_opt.is_none() {
        warn!("No broadcaster Twitch Helix credential found => cannot sync redeems to/from Helix.");
        return Ok(());
    }
    let broadcaster_cred = broadcaster_cred_opt.unwrap();

    // 2) Build a Helix client for them
    let (client_id, token) = match &broadcaster_cred.additional_data {
        Some(json) => {
            let cid_opt = json.get("client_id").and_then(|v| v.as_str());
            if let Some(cid) = cid_opt {
                (cid.to_string(), broadcaster_cred.primary_token.clone())
            } else {
                warn!("Broadcaster credential missing client_id in .additional_data => skipping.");
                return Ok(());
            }
        }
        None => {
            warn!("Broadcaster credential has no .additional_data => cannot build Helix client.");
            return Ok(());
        }
    };

    let client = TwitchHelixClient::new(&token, &client_id);

    // 3) Validate the token => get broadcaster_id
    let val = match client.validate_token().await {
        Ok(Some(info)) => info,
        Ok(None) => {
            warn!("Broadcaster credential invalid => skipping redeem sync.");
            return Ok(());
        }
        Err(e) => {
            error!("Error calling /validate => {e:?}");
            return Ok(());
        }
    };
    let broadcaster_id = val.user_id;

    // 4) Fetch the existing list from Helix (both states: is_enabled / disabled)
    //    This returns *all* rewards for that channel.
    let all_rewards = client.get_custom_rewards(&broadcaster_id, None, false).await.unwrap_or_default();
    let manage_rewards = client.get_custom_rewards(&broadcaster_id, None, true).await.unwrap_or_default();
    debug!("get_custom_rewards => total={}, manageable={}", all_rewards.len(), manage_rewards.len());

    // ----------------------------------------------------------------------------
    // PHASE A: Import from Helix -> DB
    // ----------------------------------------------------------------------------
    // For each Helix reward, check if we already have a DB row by that reward_id.
    // If not found, we create a new DB row with is_managed=false, or set is_managed=true
    // if it also appears in the “only_manageable_rewards” list.
    // We also mirror cost, is_active, etc.
    // ----------------------------------------------------------------------------

    for helix_rd in &all_rewards {
        // See if this reward already exists in DB by reward_id.
        let existing = redeem_service
            .redeem_repo
            .get_redeem_by_reward_id("twitch-eventsub", &helix_rd.id)
            .await?;

        if existing.is_none() {
            let is_in_manage_list = manage_rewards.iter().any(|mr| mr.id == helix_rd.id);

            // Insert brand-new DB row
            let new_redeem = Redeem {
                redeem_id: Uuid::new_v4(),
                platform: "twitch-eventsub".to_string(),
                reward_id: helix_rd.id.clone(),
                reward_name: helix_rd.title.clone(),
                cost: helix_rd.cost as i32,
                is_active: helix_rd.is_enabled,
                dynamic_pricing: false,
                active_offline: false,
                is_managed: is_in_manage_list, // or false if you prefer
                plugin_name: None,
                command_name: None,
                created_at: Utc::now(),
                updated_at: Utc::now(),
                active_credential_id: None,
            };

            if let Err(e) = redeem_service.redeem_repo.create_redeem(&new_redeem).await {
                error!("Failed inserting new DB redeem for Helix reward '{}': {e}", helix_rd.title);
            } else {
                info!(
                    "Imported Helix reward '{}' into local DB => is_managed={}",
                    helix_rd.title, is_in_manage_list
                );
            }
        }
    }

    // ----------------------------------------------------------------------------
    // PHASE B: For each DB redeem that is flagged is_managed=true, ensure it’s in Helix
    // ----------------------------------------------------------------------------
    let all_db_redeems = redeem_service.list_redeems("twitch-eventsub").await?;
    for rd in &all_db_redeems {
        // Only proceed if it’s “managed” by us.
        if !rd.is_managed {
            trace!(
                "Redeem '{}' => not is_managed => skipping Helix update",
                rd.reward_name
            );
            continue;
        }

        // For Helix creation or patch, you can either:
        //  (A) use the same broadcaster_cred (like we are doing)
        //  (B) or check rd.active_credential_id if you have that system in place
        // Here, for simplicity, we just do it from the broadcaster.

        sync_one_redeem_via_helix(rd, &client, &broadcaster_id, redeem_service).await?;
    }

    Ok(())
}

/// Sync exactly one redeem (rd) from DB => Helix if is_managed=true:
/// - If `rd.reward_id` is empty or not found in Helix’s list, we try create_custom_reward.
/// - If cost or “is_active” mismatch, we patch it.
async fn sync_one_redeem_via_helix(
    rd: &Redeem,
    client: &TwitchHelixClient,
    broadcaster_id: &str,
    redeem_service: &RedeemService,
) -> Result<(), Error> {
    // 1) fetch full reward list again (you can optimize by passing it in)
    let all_rewards = client.get_custom_rewards(broadcaster_id, None, false).await.unwrap_or_default();

    // 2) see if Helix already has it by reward_id
    let maybe_helix_rd = all_rewards.iter().find(|r| r.id == rd.reward_id);

    if maybe_helix_rd.is_none() {
        // Possibly Helix was missing that reward. Let’s see if we can unify by name
        // if the reward_id is empty. (We do this only if reward_id was never set.)
        if rd.reward_id.trim().is_empty() {
            // Attempt to create
            let body = CustomRewardBody {
                title: Some(rd.reward_name.clone()),
                cost: Some(rd.cost as u64),
                is_enabled: Some(rd.is_active),
                ..Default::default()
            };
            match client.create_custom_reward(broadcaster_id, &body).await {
                Ok(created) => {
                    // update DB to store the new Helix ID
                    let mut updated_rd = rd.clone();
                    updated_rd.reward_id = created.id;
                    updated_rd.updated_at = Utc::now();
                    redeem_service.redeem_repo.update_redeem(&updated_rd).await?;
                    info!("Created new Helix reward for '{}' => new ID={}", rd.reward_name, updated_rd.reward_id);
                }
                Err(e) => {
                    warn!("create_custom_reward => {e}");
                }
            }
        } else {
            // If reward_id is set but Helix does not have it, we try to create
            debug!("No Helix reward matching id='{}' => attempting create", rd.reward_id);
            let body = CustomRewardBody {
                title: Some(rd.reward_name.clone()),
                cost: Some(rd.cost as u64),
                is_enabled: Some(rd.is_active),
                ..Default::default()
            };
            match client.create_custom_reward(broadcaster_id, &body).await {
                Ok(created) => {
                    let mut updated_rd = rd.clone();
                    updated_rd.reward_id = created.id;
                    updated_rd.updated_at = Utc::now();
                    redeem_service.redeem_repo.update_redeem(&updated_rd).await?;
                    info!("Created Helix reward => updated DB for '{}'", rd.reward_name);
                }
                Err(e) => {
                    warn!(
                        "create_custom_reward => error for DB redeem '{}' => {e}",
                        rd.reward_name
                    );
                }
            }
        }
    } else {
        // Helix reward does exist, check if we need to patch cost or enabled
        let hrew = maybe_helix_rd.unwrap();
        let cost_mismatch = (rd.cost as u64) != hrew.cost;
        let active_mismatch = rd.is_active != hrew.is_enabled;

        if cost_mismatch || active_mismatch {
            debug!(
                "Patching Helix => cost {}->{}, enabled {}->{}",
                hrew.cost, rd.cost, hrew.is_enabled, rd.is_active
            );
            let body = CustomRewardBody {
                cost: if cost_mismatch { Some(rd.cost as u64) } else { None },
                is_enabled: if active_mismatch { Some(rd.is_active) } else { None },
                ..Default::default()
            };
            if let Err(e) = client.update_custom_reward(broadcaster_id, &rd.reward_id, &body).await {
                error!("update_custom_reward => {e}");
            }
        }
    }

    Ok(())
}