use std::sync::Arc;
use chrono::{Duration, Utc};
use tokio::sync::RwLock;
use tokio::time;
use tracing::{info, warn, error};
use uuid::Uuid;
use maowbot_common::{
    error::Error,
    models::osc_toggle::{OscTrigger, OscToggleState, OscParameterValue},
    traits::osc_toggle_traits::OscToggleRepository,
};
use maowbot_osc::MaowOscManager;

pub struct OscToggleService {
    osc_manager: Arc<RwLock<Option<MaowOscManager>>>,
    toggle_repo: Arc<dyn OscToggleRepository>,
}


impl OscToggleService {
    pub fn new(
        osc_manager: Arc<RwLock<Option<MaowOscManager>>>,
        toggle_repo: Arc<dyn OscToggleRepository>,
    ) -> Self {
        Self {
            osc_manager,
            toggle_repo,
        }
    }
    
    pub async fn activate_toggle(
        &self,
        redeem_id: Uuid,
        user_id: Uuid,
        avatar_id: Option<String>,
    ) -> Result<(), Error> {
        // Get trigger configuration
        let trigger = self.toggle_repo.get_trigger_by_redeem_id(redeem_id).await?
            .ok_or_else(|| Error::NotFound(format!("No OSC trigger configured for redeem {}", redeem_id)))?;
        
        if !trigger.enabled {
            return Err(Error::ValidationError("OSC trigger is disabled".to_string()));
        }
        
        // Check for active toggles if there's a cooldown
        if trigger.cooldown_seconds > 0 {
            let active_toggles = self.toggle_repo.get_active_toggles(user_id).await?;
            if active_toggles.iter().any(|t| t.trigger_id == trigger.id) {
                return Err(Error::ValidationError("Toggle is still on cooldown".to_string()));
            }
        }
        
        // Parse the on value
        let on_value = OscParameterValue::from_string(&trigger.parameter_type, &trigger.on_value)
            .map_err(|e| Error::ValidationError(e))?;
        
        // Send OSC message to turn on the toggle
        self.send_osc_parameter(&trigger.parameter_name, on_value).await?;
        
        // Create toggle state record
        let expires_at = trigger.duration_seconds.map(|seconds| {
            Utc::now() + Duration::seconds(seconds as i64)
        });
        
        let state = OscToggleState {
            id: 0, // Will be set by database
            trigger_id: trigger.id,
            user_id,
            avatar_id,
            activated_at: Utc::now(),
            expires_at,
            is_active: true,
        };
        
        let created_state = self.toggle_repo.create_toggle_state(state).await?;
        
        // If there's a duration, schedule the toggle to turn off
        if let Some(duration_seconds) = trigger.duration_seconds {
            let toggle_service = self.clone();
            let trigger_clone = trigger.clone();
            let state_id = created_state.id;
            
            tokio::spawn(async move {
                time::sleep(time::Duration::from_secs(duration_seconds as u64)).await;
                if let Err(e) = toggle_service.deactivate_toggle(state_id, &trigger_clone).await {
                    error!("Failed to deactivate toggle {}: {}", state_id, e);
                }
            });
        }
        
        info!("Activated OSC toggle {} for user {}", trigger.parameter_name, user_id);
        Ok(())
    }
    
    pub async fn deactivate_toggle(&self, state_id: i32, trigger: &OscTrigger) -> Result<(), Error> {
        // Parse the off value
        let off_value = OscParameterValue::from_string(&trigger.parameter_type, &trigger.off_value)
            .map_err(|e| Error::ValidationError(e))?;
        
        // Send OSC message to turn off the toggle
        self.send_osc_parameter(&trigger.parameter_name, off_value).await?;
        
        // Mark toggle as inactive
        self.toggle_repo.deactivate_toggle(state_id).await?;
        
        info!("Deactivated OSC toggle {}", trigger.parameter_name);
        Ok(())
    }
    
    pub async fn cleanup_expired_toggles(&self) -> Result<(), Error> {
        let expired_toggles = self.toggle_repo.get_expired_toggles().await?;
        
        for toggle_state in expired_toggles {
            if let Ok(Some(trigger)) = self.toggle_repo.get_trigger_by_id(toggle_state.trigger_id).await {
                if let Err(e) = self.deactivate_toggle(toggle_state.id, &trigger).await {
                    error!("Failed to deactivate expired toggle {}: {}", toggle_state.id, e);
                }
            }
        }
        
        Ok(())
    }
    
    async fn send_osc_parameter(&self, parameter_name: &str, value: OscParameterValue) -> Result<(), Error> {
        let osc_guard = self.osc_manager.read().await;
        if let Some(osc_manager) = osc_guard.as_ref() {
            match value {
                OscParameterValue::Bool(v) => {
                    osc_manager.send_avatar_parameter_bool(parameter_name, v)
                        .map_err(|e| Error::ServiceError(format!("Failed to send OSC bool: {}", e)))?;
                }
                OscParameterValue::Int(v) => {
                    osc_manager.send_avatar_parameter_int(parameter_name, v)
                        .map_err(|e| Error::ServiceError(format!("Failed to send OSC int: {}", e)))?;
                }
                OscParameterValue::Float(v) => {
                    osc_manager.send_avatar_parameter_float(parameter_name, v)
                        .map_err(|e| Error::ServiceError(format!("Failed to send OSC float: {}", e)))?;
                }
            }
            Ok(())
        } else {
            Err(Error::ServiceError("OSC manager not initialized".to_string()))
        }
    }
}

impl Clone for OscToggleService {
    fn clone(&self) -> Self {
        Self {
            osc_manager: Arc::clone(&self.osc_manager),
            toggle_repo: Arc::clone(&self.toggle_repo),
        }
    }
}