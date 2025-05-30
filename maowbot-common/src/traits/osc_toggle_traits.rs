use async_trait::async_trait;
use chrono::{DateTime, Utc};
use uuid::Uuid;
use crate::error::Error;
use crate::models::osc_toggle::{OscTrigger, OscToggleState, OscAvatarConfig};

#[async_trait]
pub trait OscToggleRepository: Send + Sync {
    // OscTrigger methods
    async fn get_trigger_by_id(&self, id: i32) -> Result<Option<OscTrigger>, Error>;
    async fn get_trigger_by_redeem_id(&self, redeem_id: Uuid) -> Result<Option<OscTrigger>, Error>;
    async fn get_all_triggers(&self) -> Result<Vec<OscTrigger>, Error>;
    async fn create_trigger(&self, trigger: OscTrigger) -> Result<OscTrigger, Error>;
    async fn update_trigger(&self, trigger: OscTrigger) -> Result<OscTrigger, Error>;
    async fn delete_trigger(&self, id: i32) -> Result<(), Error>;
    
    // OscToggleState methods
    async fn get_active_toggles(&self, user_id: Uuid) -> Result<Vec<OscToggleState>, Error>;
    async fn get_expired_toggles(&self) -> Result<Vec<OscToggleState>, Error>;
    async fn create_toggle_state(&self, state: OscToggleState) -> Result<OscToggleState, Error>;
    async fn deactivate_toggle(&self, id: i32) -> Result<(), Error>;
    async fn cleanup_expired_toggles(&self) -> Result<i64, Error>;
    
    // OscAvatarConfig methods
    async fn get_avatar_config(&self, avatar_id: &str) -> Result<Option<OscAvatarConfig>, Error>;
    async fn create_or_update_avatar_config(&self, config: OscAvatarConfig) -> Result<OscAvatarConfig, Error>;
}