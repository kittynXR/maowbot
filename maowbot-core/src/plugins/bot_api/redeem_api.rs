use async_trait::async_trait;
use uuid::Uuid;
use crate::Error;
use crate::models::{Redeem, RedeemUsage};

/// A sub-trait for managing channel points redeems from external clients.
#[async_trait]
pub trait RedeemApi: Send + Sync {
    async fn list_redeems(&self, platform: &str) -> Result<Vec<Redeem>, Error>;
    async fn create_redeem(&self, platform: &str, reward_id: &str, reward_name: &str, cost: i32, dynamic: bool)
                           -> Result<Redeem, Error>;
    async fn set_redeem_active(&self, redeem_id: Uuid, is_active: bool) -> Result<(), Error>;
    async fn update_redeem_cost(&self, redeem_id: Uuid, new_cost: i32) -> Result<(), Error>;
    async fn delete_redeem(&self, redeem_id: Uuid) -> Result<(), Error>;

    // Usage logs
    async fn get_usage_for_redeem(&self, redeem_id: Uuid, limit: i64) -> Result<Vec<RedeemUsage>, Error>;
    async fn get_usage_for_user(&self, user_id: Uuid, limit: i64) -> Result<Vec<RedeemUsage>, Error>;
    async fn update_redeem(&self, redeem: &Redeem) -> Result<(), Error>;
    async fn sync_redeems(&self) -> Result<(), Error>;
}