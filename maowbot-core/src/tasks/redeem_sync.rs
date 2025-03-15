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

/// The main function to sync local DB redeems to Twitch Helix.
/// In the new scheme, each `redeem.active_credential_id` points to the channel
/// that “owns” the redeem. If that credential is not present or not Helix,
/// we skip. If `active_credential_id` is absent, we skip or use older fallback.
pub async fn sync_channel_redeems(
    redeem_service: &RedeemService,
    platform_manager: &PlatformManager,
    user_service: &UserService,
    bot_config_repo: &dyn BotConfigRepository,
    is_stream_online: bool,
) -> Result<(), Error> {
    info!("Redeem sync started => is_stream_online={}", is_stream_online);

    // 1) gather all DB redeems
    let all_redeems = redeem_service.list_redeems("twitch-eventsub").await?;
    info!(
        "[redeem_sync] Found {} DB redeems on platform='twitch-eventsub'.",
        all_redeems.len()
    );

    // 2) We now attempt per redeem, based on its .active_credential_id
    for rd in &all_redeems {
        match rd.active_credential_id {
            Some(cid) => {
                // find the credential
                if let Ok(Some(cred)) = redeem_service.credentials_repo.get_credential_by_id(cid).await {
                    if cred.platform == Platform::Twitch {
                        sync_one_redeem_via_helix(rd, &cred, redeem_service).await?;
                    } else {
                        debug!("redeem '{}' active_credential is not Helix => skipping", rd.reward_name);
                    }
                } else {
                    debug!("redeem '{}' missing valid credential for active_credential_id={}", rd.reward_name, cid);
                }
            }
            None => {
                // old fallback if you still want to unify or skip
                debug!("redeem '{}' has no active_credential_id => skipping Helix sync", rd.reward_name);
            }
        }
    }

    // *Additionally*, if you still want older logic that updates
    // them on a “global” broadcaster channel, you can keep that
    // code here. For brevity, we skip it in the example.

    Ok(())
}

/// Sync exactly one redeem (rd) to the Helix channel from a given Helix credential.
async fn sync_one_redeem_via_helix(
    rd: &Redeem,
    cred: &maowbot_common::models::platform::PlatformCredential,
    redeem_service: &RedeemService,
) -> Result<(), Error> {
    // 1) Build Helix client
    let (client_id, token) = match &cred.additional_data {
        Some(json) => {
            let cid_opt = json.get("client_id").and_then(|v| v.as_str());
            if let Some(cid) = cid_opt {
                (cid.to_string(), cred.primary_token.clone())
            } else {
                return Ok(());
            }
        }
        None => return Ok(()),
    };
    let client = TwitchHelixClient::new(&token, &client_id);

    // 2) Validate
    let val = match client.validate_token().await {
        Ok(Some(info)) => info,
        Ok(None) => {
            warn!("Credential user_id={} invalid or no user => skipping", cred.user_id);
            return Ok(());
        }
        Err(e) => {
            error!("Error calling /validate => {e:?}");
            return Ok(());
        }
    };
    let broadcaster_id = val.user_id;

    // 3) get the existing list from Helix
    let all_rewards = client.get_custom_rewards(&broadcaster_id, None, false).await.unwrap_or_default();
    let manage_rewards = client.get_custom_rewards(&broadcaster_id, None, true).await.unwrap_or_default();

    let is_already_managed = manage_rewards.iter().any(|mr| mr.id == rd.reward_id);

    // If the user had it flagged as `is_managed = true` but it’s not in Helix, we create
    // (or unify by name).
    if rd.is_managed && !is_in_list(&manage_rewards, &rd.reward_id) {
        // Attempt unify by name if possible:
        if rd.reward_id.trim().is_empty() {
            // create
            let body = CustomRewardBody {
                title: Some(rd.reward_name.clone()),
                cost: Some(rd.cost as u64),
                is_enabled: Some(rd.is_active),
                ..Default::default()
            };
            if let Ok(created) = client.create_custom_reward(&broadcaster_id, &body).await {
                let mut updated = rd.clone();
                updated.reward_id = created.id;
                updated.updated_at = Utc::now();
                redeem_service.redeem_repo.update_redeem(&updated).await?;
                info!("Created Helix reward for '{}' => new ID={}", rd.reward_name, updated.reward_id);
            }
        } else {
            // see if Helix has a reward with that name
            if let Some(rid) = find_reward_id_by_title_ignorecase(&all_rewards, &rd.reward_name) {
                let mut updated = rd.clone();
                updated.reward_id = rid;
                updated.updated_at = Utc::now();
                redeem_service.redeem_repo.update_redeem(&updated).await?;
                info!("Unify by name => updated reward_id for '{}'", rd.reward_name);
            }
            // else no easy unify, might do fallback
        }
    }

    // If cost or active mismatch, patch
    if let Some(hrew) = all_rewards.iter().find(|r| r.id == rd.reward_id) {
        let cost_mismatch = (rd.cost as u64) != hrew.cost;
        let active_mismatch = rd.is_active != hrew.is_enabled;
        if cost_mismatch || active_mismatch {
            debug!(
                "[sync_one] patching => cost={} vs {}, active={} vs {}",
                rd.cost, hrew.cost, rd.is_active, hrew.is_enabled
            );
            let body = CustomRewardBody {
                cost: if cost_mismatch { Some(rd.cost as u64) } else { None },
                is_enabled: if active_mismatch { Some(rd.is_active) } else { None },
                ..Default::default()
            };
            if let Err(e) = client.update_custom_reward(&broadcaster_id, &rd.reward_id, &body).await {
                error!("update_custom_reward => {e}");
            }
        }
    } else {
        debug!("No matching Helix reward for '{}', id='{}'", rd.reward_name, rd.reward_id);
    }

    Ok(())
}
