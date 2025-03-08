use std::sync::Arc;
use chrono::{Utc};
use uuid::Uuid;
use tracing::{info, error, warn};
use crate::Error;
use crate::models::{Redeem, RedeemUsage};
use crate::repositories::{
    RedeemRepository, RedeemUsageRepository,
};
use crate::services::user_service::UserService;

/// Manages channel-point redeems (Channel Points) from Twitch EventSub or other platforms.
/// - We can dynamically update the cost, enable/disable certain redeems, etc.
/// - Whenever a redemption is triggered from EventSub, we log usage and apply any custom logic.
pub struct RedeemService {
    redeem_repo: Arc<dyn RedeemRepository + Send + Sync>,
    usage_repo: Arc<dyn RedeemUsageRepository + Send + Sync>,
    user_service: Arc<UserService>,
}

impl RedeemService {
    pub fn new(
        redeem_repo: Arc<dyn RedeemRepository + Send + Sync>,
        usage_repo: Arc<dyn RedeemUsageRepository + Send + Sync>,
        user_service: Arc<UserService>,
    ) -> Self {
        Self {
            redeem_repo,
            usage_repo,
            user_service,
        }
    }

    /// Called when we receive a channel point redemption via EventSub.
    /// `reward_id` is the unique Twitch reward identifier, `user_id` is our DB user, and
    /// `channel` is the channel or broadcaster context, if needed.
    pub async fn handle_redeem(
        &self,
        platform: &str,
        reward_id: &str,
        user_id: Uuid,
        channel: &str,
        usage_data: Option<serde_json::Value>,
    ) -> Result<(), Error> {
        // find the Redeem row
        let r_opt = self.redeem_repo
            .get_redeem_by_reward_id(platform, reward_id)
            .await?;
        let rd = match r_opt {
            Some(r) => r,
            None => {
                warn!("No matching Redeem config found for reward_id='{}' platform='{}'", reward_id, platform);
                return Ok(()); // or an error
            }
        };

        if !rd.is_active {
            warn!("Redeem '{}' is not active", rd.reward_name);
            return Ok(());
        }

        // Possibly handle dynamic cost changes:
        if rd.dynamic_pricing {
            // Example logic: each redemption raises cost by 100, or something else.
            let new_cost = rd.cost + 100;
            let mut updated_rd = rd.clone();
            updated_rd.cost = new_cost;
            updated_rd.updated_at = Utc::now();
            if let Err(e) = self.redeem_repo.update_redeem(&updated_rd).await {
                error!("Failed to update dynamic pricing => {:?}", e);
            } else {
                info!("Dynamic pricing updated for redeem '{}' => new cost={}", updated_rd.reward_name, new_cost);
            }
        }

        // log usage
        let usage = RedeemUsage {
            usage_id: Uuid::new_v4(),
            redeem_id: rd.redeem_id,
            user_id,
            used_at: Utc::now(),
            channel: Some(channel.to_string()),
            usage_data,
        };
        self.usage_repo.insert_usage(&usage).await?;

        // [Optionally] custom logic...
        info!("User {:?} redeemed '{}' in channel '{}'", user_id, rd.reward_name, channel);

        Ok(())
    }

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
