use std::sync::Arc;
use async_trait::async_trait;
use uuid::Uuid;
use crate::Error;
use crate::models::{Redeem, RedeemUsage};
use crate::plugins::bot_api::redeem_api::RedeemApi;
use crate::plugins::manager::core::PluginManager;
use crate::repositories::{
    RedeemUsageRepository,
};
use crate::tasks::redeem_sync;

#[async_trait]
impl RedeemApi for PluginManager {
    async fn list_redeems(&self, platform: &str) -> Result<Vec<Redeem>, Error> {
        let rsvc = self.resolve_redeem_service()?;
        rsvc.list_redeems(platform).await
    }

    async fn create_redeem(&self, platform: &str, reward_id: &str, reward_name: &str, cost: i32, dynamic: bool)
                           -> Result<Redeem, Error>
    {
        let rsvc = self.resolve_redeem_service()?;
        rsvc.create_redeem(platform, reward_id, reward_name, cost, dynamic).await
    }

    async fn set_redeem_active(&self, redeem_id: Uuid, is_active: bool) -> Result<(), Error> {
        let rsvc = self.resolve_redeem_service()?;
        rsvc.set_redeem_active(redeem_id, is_active).await
    }

    async fn update_redeem_cost(&self, redeem_id: Uuid, new_cost: i32) -> Result<(), Error> {
        let rsvc = self.resolve_redeem_service()?;
        rsvc.update_redeem_cost(redeem_id, new_cost).await
    }

    async fn delete_redeem(&self, redeem_id: Uuid) -> Result<(), Error> {
        let rsvc = self.resolve_redeem_service()?;
        rsvc.delete_redeem(redeem_id).await
    }

    async fn get_usage_for_redeem(&self, redeem_id: Uuid, limit: i64) -> Result<Vec<RedeemUsage>, Error> {
        let usage_repo = match &self.redeem_usage_repo {
            repo => repo.clone(),
        };
        usage_repo.list_usage_for_redeem(redeem_id, limit).await
    }

    async fn get_usage_for_user(&self, user_id: Uuid, limit: i64) -> Result<Vec<RedeemUsage>, Error> {
        let usage_repo = match &self.redeem_usage_repo {
            repo => repo.clone(),
        };
        usage_repo.list_usage_for_user(user_id, limit).await
    }

    async fn update_redeem(&self, redeem: &Redeem) -> Result<(), Error> {
        let rsvc = self.resolve_redeem_service()?;  // Possibly calls `self.redeem_service.clone()`
        rsvc.redeem_repo.update_redeem(redeem).await
    }

    async fn sync_redeems(&self) -> Result<(), Error> {
        // For example, run the actual sync logic:
        redeem_sync::sync_channel_redeems(
            &self.redeem_service,
            &self.platform_manager,
            &self.user_service,
            self.command_service.bot_config_repo.as_ref(),
            false
        ).await
    }
}

impl PluginManager {
    pub fn resolve_redeem_service(&self) -> Result<Arc<crate::services::RedeemService>, Error> {
        match &self.redeem_service {
            svc => Ok(svc.clone()),
        }
    }
}
