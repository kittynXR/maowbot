// File: maowbot-core/src/services/twitch/redeem_service.rs

use std::sync::Arc;
use chrono::{Utc};
use uuid::Uuid;
use tracing::{info, error, warn, debug};
use crate::Error;
pub(crate) use crate::models::{Redeem, RedeemUsage};
use crate::repositories::{
    RedeemRepository, RedeemUsageRepository,
};
use crate::services::user_service::UserService;
use crate::platforms::manager::PlatformManager;
use crate::platforms::twitch::requests::channel_points::{Redemption};
use crate::services::twitch::builtin_redeems;

/// A struct containing references that each builtin redeem submodule might need:
///   - A Helix client for updating redemption status,
///   - The main `RedeemService` reference, etc.
pub struct RedeemHandlerContext<'a> {
    /// Possibly the Helix client for the active broadcaster, if we want to
    /// call Twitch API to reject or fulfill the redemption, etc.
    pub helix_client: Option<crate::platforms::twitch::client::TwitchHelixClient>,

    /// Reference to the RedeemService for any additional method calls
    pub redeem_service: &'a RedeemService,
}

pub struct RedeemService {
    pub(crate) redeem_repo: Arc<dyn RedeemRepository + Send + Sync>,
    usage_repo: Arc<dyn RedeemUsageRepository + Send + Sync>,
    user_service: Arc<UserService>,

    /// So we can fetch a Helix client to update redemptions, if needed.
    platform_manager: Arc<PlatformManager>,
}

impl RedeemService {
    pub fn new(
        redeem_repo: Arc<dyn RedeemRepository + Send + Sync>,
        usage_repo: Arc<dyn RedeemUsageRepository + Send + Sync>,
        user_service: Arc<UserService>,
        platform_manager: Arc<PlatformManager>,
    ) -> Self {
        Self {
            redeem_repo,
            usage_repo,
            user_service,
            platform_manager,
        }
    }

    /// Called by our Twitch EventSub pipeline whenever a new redemption event arrives.
    /// `reward_id`: the unique Twitch reward ID. We match it to the `redeems.reward_id`.
    /// `user_id`: the user who redeemed it (our DB user_id).
    /// `channel`: the channel name or broadcaster name context, if needed
    /// `redemption`: the entire data from EventSub with user_input, redemption_id, etc.
    ///
    /// This method:
    ///   1. Finds the matching DB row from `redeems`.
    ///   2. Logs usage in `redeem_usage`.
    ///   3. Builds a `RedeemHandlerContext` and calls the appropriate built-in or plugin logic.
    pub async fn handle_incoming_redeem(
        &self,
        platform: &str,
        reward_id: &str,
        user_id: Uuid,
        channel: &str,
        redemption: &Redemption,
    ) -> Result<(), Error> {
        // 1) Look up the Redeem row
        let rd_opt = self.redeem_repo
            .get_redeem_by_reward_id(platform, reward_id)
            .await?;
        let rd = match rd_opt {
            Some(r) => r,
            None => {
                warn!("No matching Redeem found for platform='{}' reward_id='{}'", platform, reward_id);
                return Ok(()); // We do nothing if it's unknown
            }
        };

        if !rd.is_active {
            debug!("Redeem '{}' is inactive => ignoring event", rd.reward_name);
            return Ok(());
        }

        // 2) Log usage in DB
        let usage = RedeemUsage {
            usage_id: Uuid::new_v4(),
            redeem_id: rd.redeem_id,
            user_id,
            used_at: Utc::now(),
            channel: Some(channel.to_string()),
            usage_data: None, // You can store redemption.user_input here or other fields
        };
        self.usage_repo.insert_usage(&usage).await?;

        // 3) Build the handler context
        let ctx = RedeemHandlerContext {
            helix_client: self.get_helix_client_for_broadcaster(platform, channel).await,
            redeem_service: self,
        };

        // 4) Decide if it’s built-in or plugin-based
        if rd.plugin_name.is_some() {
            // In the future, dispatch to plugin
            // e.g. self.call_plugin(rd.plugin_name, redemption)
            // (not yet implemented)
            info!("Redeem '{}' is plugin-based => not yet implemented logic.", rd.reward_name);
        } else {
            // Built-in logic by reward_name:
            builtin_redeems::handle_builtin_redeem(&ctx, redemption, &rd.reward_name).await?;
        }

        Ok(())
    }

    /// Example helper that tries to find a Helix credential for the broadcaster,
    /// then constructs a TwitchHelixClient from it. If none found, we return None.
    async fn get_helix_client_for_broadcaster(
        &self,
        platform: &str,
        channel_or_broadcaster: &str,
    ) -> Option<crate::platforms::twitch::client::TwitchHelixClient> {
        // For simplicity, assume we have only one “account_name” that matches
        // the broadcaster’s global_username. Then ask platform_manager
        // to see if it has a Helix token. This is quite custom to your logic.
        let user_result = self.user_service
            .find_user_by_global_username(channel_or_broadcaster)
            .await;
        let user = match user_result {
            Ok(u) => u,
            Err(_) => {
                debug!("No matching local user for broadcaster='{}'", channel_or_broadcaster);
                return None;
            }
        };

        // Then we get the Helix client from the platform manager or from the credentials.
        let key = ("twitch".to_string(), user.user_id.to_string());
        let guard = self.platform_manager.active_runtimes.lock().await;
        let handle_opt = guard.get(&key);
        if let Some(handle) = handle_opt {
            // We can try to see if handle has a Helix client if we stored it
            // e.g. if handle.platform == "twitch"
            // but in this example, we do not store it directly. We'll reconstruct from credentials.
        }
        drop(guard);

        // Reconstruct from the credential:
        // (You might keep repeated code in a helper.)
        if let Ok(Some(cred)) = self.platform_manager.credentials_repo
            .get_credentials(&crate::models::Platform::Twitch, user.user_id)
            .await
        {
            if let Some(additional) = &cred.additional_data {
                if let Some(cid) = additional.get("client_id").and_then(|v| v.as_str()) {
                    let client = crate::platforms::twitch::client::TwitchHelixClient::new(
                        &cred.primary_token,
                        cid
                    );
                    return Some(client);
                }
            }
        }
        None
    }

    // ------------------------------------------------------------------
    // Additional CRUD / sync logic
    // ------------------------------------------------------------------

    pub async fn create_redeem(
        &self,
        platform: &str,
        reward_id: &str,
        reward_name: &str,
        cost: i32,
        dynamic: bool
    ) -> Result<Redeem, Error> {
        let now = Utc::now();
        let rd = Redeem {
            redeem_id: Uuid::new_v4(),
            platform: platform.to_string(),
            reward_id: reward_id.to_string(),
            reward_name: reward_name.to_string(),
            cost,
            is_active: true,
            dynamic_pricing: dynamic,
            created_at: now,
            updated_at: now,
            // new columns with defaults:
            active_offline: false,
            is_managed: false,
            plugin_name: None,
            command_name: None,
        };
        self.redeem_repo.create_redeem(&rd).await?;
        Ok(rd)
    }

    pub async fn list_redeems(&self, platform: &str) -> Result<Vec<Redeem>, Error> {
        self.redeem_repo.list_redeems(platform).await
    }

    pub async fn update_redeem_cost(&self, redeem_id: Uuid, new_cost: i32) -> Result<(), Error> {
        if let Some(mut r) = self.redeem_repo.get_redeem_by_id(redeem_id).await? {
            r.cost = new_cost;
            r.updated_at = Utc::now();
            self.redeem_repo.update_redeem(&r).await?;
        }
        Ok(())
    }

    pub async fn set_redeem_active(&self, redeem_id: Uuid, is_active: bool) -> Result<(), Error> {
        if let Some(mut r) = self.redeem_repo.get_redeem_by_id(redeem_id).await? {
            r.is_active = is_active;
            r.updated_at = Utc::now();
            self.redeem_repo.update_redeem(&r).await?;
        }
        Ok(())
    }

    pub async fn delete_redeem(&self, redeem_id: Uuid) -> Result<(), Error> {
        self.redeem_repo.delete_redeem(redeem_id).await
    }

    /// Example function to ensure all built-in redeems exist, or are updated as needed.
    /// You can call this on bot startup, channel update, stream online/offline, etc.
    pub async fn sync_built_in_redeems(&self) -> Result<(), Error> {
        info!("sync_built_in_redeems: verifying built-in redeems exist in DB...");

        // For demonstration, we only “require” the “cute” redeem to exist. If it’s missing, we insert it.
        let found = self.redeem_repo.get_redeem_by_reward_id("twitch-eventsub", "builtin.cute").await?;
        if found.is_none() {
            info!("'cute' redeem missing => creating new row in DB...");
            let now = Utc::now();
            let new_rd = Redeem {
                redeem_id: Uuid::new_v4(),
                platform: "twitch-eventsub".to_string(),
                reward_id: "builtin.cute".to_string(),
                reward_name: "cute".to_string(),
                cost: 50,
                is_active: true,
                dynamic_pricing: false,
                created_at: now,
                updated_at: now,
                active_offline: true,
                is_managed: true,
                plugin_name: None,
                command_name: None,
            };
            self.redeem_repo.create_redeem(&new_rd).await?;
        } else {
            debug!("Found 'cute' redeem => no changes.");
        }

        // If you had more built-in redeems, check them all here.

        Ok(())
    }
}
