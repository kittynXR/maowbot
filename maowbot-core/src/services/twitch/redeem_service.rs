use std::sync::Arc;
use chrono::{Utc};
use uuid::Uuid;
use tracing::{info, warn, debug};
use maowbot_common::models::platform::{Platform, PlatformCredential};
use maowbot_common::models::{Redeem, RedeemUsage};
use maowbot_common::traits::repository_traits::{RedeemRepository, RedeemUsageRepository, CredentialsRepository};
use crate::Error;
use crate::services::user_service::UserService;
use crate::platforms::manager::PlatformManager;
use crate::platforms::twitch::requests::channel_points::Redemption;
use crate::services::twitch::builtin_redeems;

/// Holds references needed in a built‑in redeem flow:
pub struct RedeemHandlerContext<'a> {
    /// Possibly the Helix client if we want to auto-accept or decline the redemption, etc.
    pub helix_client: Option<crate::platforms::twitch::client::TwitchHelixClient>,

    /// Reference to the RedeemService itself in case the builtin code calls more methods.
    pub redeem_service: &'a RedeemService,

    /// **NEW**: The credential that “actually processes” the redeem, if relevant
    /// (based on `active_credential_id` fallback).
    pub active_credential: Option<PlatformCredential>,
}

pub struct RedeemService {
    pub(crate) redeem_repo: Arc<dyn RedeemRepository + Send + Sync>,
    usage_repo: Arc<dyn RedeemUsageRepository + Send + Sync>,
    user_service: Arc<UserService>,

    platform_manager: Arc<PlatformManager>,

    /// For picking fallback credentials, we also need direct access to credentials_repo
    pub credentials_repo: Arc<dyn CredentialsRepository + Send + Sync>,
}

impl RedeemService {
    pub fn new(
        redeem_repo: Arc<dyn RedeemRepository + Send + Sync>,
        usage_repo: Arc<dyn RedeemUsageRepository + Send + Sync>,
        user_service: Arc<UserService>,
        platform_manager: Arc<PlatformManager>,
        credentials_repo: Arc<dyn CredentialsRepository + Send + Sync>,
    ) -> Self {
        Self {
            redeem_repo,
            usage_repo,
            user_service,
            platform_manager,
            credentials_repo,
        }
    }

    /// Called by Twitch EventSub pipeline or a similar mechanism whenever a new redemption arrives.
    pub async fn handle_incoming_redeem(
        &self,
        platform: &str,
        reward_id: &str,
        user_id: Uuid,
        channel: &str,
        redemption: &Redemption,
    ) -> Result<(), Error> {
        let rd_opt = self.redeem_repo
            .get_redeem_by_reward_id(platform, reward_id)
            .await?;
        let rd = match rd_opt {
            Some(r) => r,
            None => {
                warn!("No matching Redeem found for platform='{}' reward_id='{}'", platform, reward_id);
                return Ok(());
            }
        };

        if !rd.is_active {
            debug!("Redeem '{}' is inactive => ignoring event", rd.reward_name);
            return Ok(());
        }

        // Log usage
        let usage = RedeemUsage {
            usage_id: Uuid::new_v4(),
            redeem_id: rd.redeem_id,
            user_id,
            used_at: Utc::now(),
            channel: Some(channel.to_string()),
            usage_data: None,
        };
        self.usage_repo.insert_usage(&usage).await?;

        // Decide which credential actually processes it => check rd.active_credential_id
        let chosen_credential = self.pick_active_redeem_credential(&rd, user_id).await?;

        // Build the handler context
        let ctx = RedeemHandlerContext {
            helix_client: self.get_helix_client_for_credential(&chosen_credential).await,
            redeem_service: self,
            active_credential: chosen_credential,
        };

        // If plugin_name is “builtin”, handle:
        if let Some(plugin) = &rd.plugin_name {
            if plugin == "builtin" {
                let subcmd = rd.command_name.as_deref().unwrap_or("unknown");
                builtin_redeems::handle_builtin_redeem(&ctx, redemption, subcmd).await?;
            } else {
                info!(
                    "Redeem '{}' => plugin_name='{}' is not builtin => skipping for now.",
                    rd.reward_name, plugin
                );
            }
        }
        // else: no plugin => ignoring

        Ok(())
    }

    /// Picks the “active credential” for processing a redeem:
    ///  1) If rd.active_credential_id is set, use it if it’s Twitch + a valid token.
    ///  2) If none, use the same fallback approach as commands:
    ///     - first bot,
    ///     - then broadcaster,
    ///     - else the same user’s own credential,
    ///     - else None if truly no credential at all.
    async fn pick_active_redeem_credential(
        &self,
        rd: &Redeem,
        redeeming_user_id: Uuid
    ) -> Result<Option<PlatformCredential>, Error> {
        // step 1
        if let Some(cid) = rd.active_credential_id {
            if let Ok(Some(c)) = self.credentials_repo.get_credential_by_id(cid).await {
                if c.platform == Platform::TwitchIRC || c.platform == Platform::Twitch {
                    return Ok(Some(c));
                }
            }
        }

        // step 2 (fallback chain)
        let all_irc = self.credentials_repo
            .list_credentials_for_platform(&Platform::TwitchIRC)
            .await?;
        // first: any bot?
        if let Some(bot) = all_irc.iter().find(|c| c.is_bot) {
            return Ok(Some(bot.clone()));
        }

        // next: any broadcaster
        if let Some(broad) = all_irc.iter().find(|c| c.is_broadcaster) {
            return Ok(Some(broad.clone()));
        }

        // next: the same user
        if let Some(uc) = all_irc.iter().find(|c| c.user_id == redeeming_user_id) {
            return Ok(Some(uc.clone()));
        }

        // none
        Ok(None)
    }

    /// Optionally build a Helix client from a given credential if it’s the Helix type.
    async fn get_helix_client_for_credential(
        &self,
        cred_opt: &Option<PlatformCredential>
    ) -> Option<crate::platforms::twitch::client::TwitchHelixClient> {
        if let Some(cred) = cred_opt {
            if cred.platform == Platform::Twitch {
                if let Some(additional) = &cred.additional_data {
                    if let Some(cid) = additional.get("client_id").and_then(|v| v.as_str()) {
                        let client = crate::platforms::twitch::client::TwitchHelixClient::new(
                            &cred.primary_token, cid
                        );
                        return Some(client);
                    }
                }
            }
        }
        None
    }

    // ------------------------------------------------------------------
    // Additional CRUD
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
            active_credential_id: None,
            platform: platform.to_string(),
            reward_id: reward_id.to_string(),
            reward_name: reward_name.to_string(),
            cost,
            is_active: true,
            dynamic_pricing: dynamic,
            created_at: now,
            updated_at: now,
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
}
