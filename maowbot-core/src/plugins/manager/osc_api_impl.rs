use crate::Error;
use maowbot_common::traits::api::{OscApi};
use maowbot_common::models::osc::{OscStatus};
use crate::plugins::manager::core::PluginManager;
use async_trait::async_trait;

#[async_trait]
impl OscApi for PluginManager {
    async fn osc_start(&self) -> Result<(), Error> {
        let mgr = self.osc_manager
            .as_ref()
            .ok_or_else(|| Error::Platform("No OSC manager attached".to_string()))?;
        
        // Load configured destinations from bot_config
        if let Some(auth_mgr) = &self.auth_manager {
            let auth_guard = auth_mgr.lock().await;
            
            // Load VRChat destination
            if let Ok(Some(vrchat_dest)) = auth_guard.bot_config_repo.get_value("osc_vrchat_dest").await {
                mgr.set_vrchat_dest(Some(vrchat_dest)).await;
            }
            
            // Load Robot destination
            if let Ok(Some(robot_dest)) = auth_guard.bot_config_repo.get_value("osc_robot_dest").await {
                mgr.set_robot_dest(Some(robot_dest)).await;
            }
        }
        
        mgr.start_all()
            .await
            .map_err(|e| Error::Platform(format!("OSC start error: {e:?}")))?;
        Ok(())
    }

    async fn osc_stop(&self) -> Result<(), Error> {
        let mgr = self.osc_manager
            .as_ref()
            .ok_or_else(|| Error::Platform("No OSC manager attached".to_string()))?;
        mgr.stop_all()
            .await
            .map_err(|e| Error::Platform(format!("OSC stop error: {e:?}")))?;
        Ok(())
    }

    async fn osc_status(&self) -> Result<OscStatus, Error> {
        if let Some(mgr) = &self.osc_manager {
            // <--- Retrieve the overall status from our new helper:
            let st = mgr.get_status()
                .await
                .map_err(|e| Error::Platform(format!("OSC status error: {e:?}")))?;

            // For the OSCQuery side:
            let oscq = mgr.oscquery_server.lock().await;
            let is_oscquery_running = oscq.is_running;  // using the accessor
            let port = oscq.http_port;

            // Return a user-friendly OscStatus
            Ok(OscStatus {
                is_running: st.is_running,
                listening_port: st.listening_port,
                is_oscquery_running,
                oscquery_port: Some(port),
                discovered_peers: Vec::new(),
            })
        } else {
            // No manager => default "off" status
            Ok(OscStatus {
                is_running: false,
                listening_port: None,
                is_oscquery_running: false,
                oscquery_port: None,
                discovered_peers: Vec::new(),
            })
        }
    }

    async fn osc_chatbox(&self, message: &str) -> Result<(), Error> {
        let mgr = self.osc_manager
            .as_ref()
            .ok_or_else(|| Error::Platform("No OSC manager attached".to_string()))?;
        let msg = maowbot_osc::vrchat::chatbox::ChatboxMessage {
            text: message.to_string(),
            send_immediately: true,
            play_notification_sound: true,
        };
        maowbot_osc::vrchat::chatbox::send_chatbox_message(mgr, &msg)
            .map_err(|e| Error::Platform(format!("OSC chat error: {e:?}")))?;
        Ok(())
    }

    async fn osc_discover_peers(&self) -> Result<Vec<String>, Error> {
        let mgr = self.osc_manager
            .as_ref()
            .ok_or_else(|| Error::Platform("No OSC manager attached".to_string()))?;
        mgr.discover_local_peers()
            .await
            .map_err(|e| Error::Platform(format!("OSC discover error: {e:?}")))
    }

    // Add the implementation for osc_take_raw_receiver:
    async fn osc_take_raw_receiver(&self) -> Result<Option<tokio::sync::mpsc::UnboundedReceiver<rosc::OscPacket>>, Error> {
        let mgr = self.osc_manager
            .as_ref()
            .ok_or_else(|| Error::Platform("No OSC manager attached".to_string()))?;

        // Now this method returns Future<Output = Option<...>>
        let receiver = mgr.take_osc_receiver().await;
        Ok(receiver)
    }
    
    async fn osc_send_avatar_parameter_bool(&self, name: &str, value: bool) -> Result<(), Error> {
        let mgr = self.osc_manager
            .as_ref()
            .ok_or_else(|| Error::Platform("No OSC manager attached".to_string()))?;
        
        // Load the latest VRChat destination from config
        if let Some(auth_mgr) = &self.auth_manager {
            let auth_guard = auth_mgr.lock().await;
            if let Ok(Some(vrchat_dest)) = auth_guard.bot_config_repo.get_value("osc_vrchat_dest").await {
                mgr.set_vrchat_dest(Some(vrchat_dest)).await;
            }
        }
        
        mgr.send_avatar_parameter_bool(name, value)
            .map_err(|e| Error::Platform(format!("OSC send bool error: {e:?}")))?;
        Ok(())
    }
    
    async fn osc_send_avatar_parameter_int(&self, name: &str, value: i32) -> Result<(), Error> {
        let mgr = self.osc_manager
            .as_ref()
            .ok_or_else(|| Error::Platform("No OSC manager attached".to_string()))?;
        
        // Load the latest VRChat destination from config
        if let Some(auth_mgr) = &self.auth_manager {
            let auth_guard = auth_mgr.lock().await;
            if let Ok(Some(vrchat_dest)) = auth_guard.bot_config_repo.get_value("osc_vrchat_dest").await {
                mgr.set_vrchat_dest(Some(vrchat_dest)).await;
            }
        }
        
        mgr.send_avatar_parameter_int(name, value)
            .map_err(|e| Error::Platform(format!("OSC send int error: {e:?}")))?;
        Ok(())
    }
    
    async fn osc_send_avatar_parameter_float(&self, name: &str, value: f32) -> Result<(), Error> {
        let mgr = self.osc_manager
            .as_ref()
            .ok_or_else(|| Error::Platform("No OSC manager attached".to_string()))?;
        
        // Load the latest VRChat destination from config
        if let Some(auth_mgr) = &self.auth_manager {
            let auth_guard = auth_mgr.lock().await;
            if let Ok(Some(vrchat_dest)) = auth_guard.bot_config_repo.get_value("osc_vrchat_dest").await {
                mgr.set_vrchat_dest(Some(vrchat_dest)).await;
            }
        }
        
        mgr.send_avatar_parameter_float(name, value)
            .map_err(|e| Error::Platform(format!("OSC send float error: {e:?}")))?;
        Ok(())
    }
    
    async fn osc_list_triggers(&self) -> Result<Vec<maowbot_common::models::osc_toggle::OscTrigger>, Error> {
        let repo = self.osc_toggle_repo
            .as_ref()
            .ok_or_else(|| Error::Platform("No OSC toggle repository attached".to_string()))?;
        repo.get_all_triggers().await
    }
    
    async fn osc_list_triggers_with_redeems(&self) -> Result<Vec<(maowbot_common::models::osc_toggle::OscTrigger, String)>, Error> {
        let repo = self.osc_toggle_repo
            .as_ref()
            .ok_or_else(|| Error::Platform("No OSC toggle repository attached".to_string()))?;
        let triggers = repo.get_all_triggers().await?;
        
        // For each trigger, get the redeem name
        let mut results = Vec::new();
        for trigger in triggers {
            if let Ok(Some(redeem)) = self.redeem_service.redeem_repo.get_redeem_by_id(trigger.redeem_id).await {
                results.push((trigger, redeem.reward_name));
            } else {
                results.push((trigger, "Unknown Redeem".to_string()));
            }
        }
        Ok(results)
    }
    
    async fn osc_get_trigger(&self, trigger_id: i32) -> Result<Option<maowbot_common::models::osc_toggle::OscTrigger>, Error> {
        let repo = self.osc_toggle_repo
            .as_ref()
            .ok_or_else(|| Error::Platform("No OSC toggle repository attached".to_string()))?;
        repo.get_trigger_by_id(trigger_id).await
    }
    
    async fn osc_create_trigger(&self, trigger: maowbot_common::models::osc_toggle::OscTrigger) -> Result<maowbot_common::models::osc_toggle::OscTrigger, Error> {
        let repo = self.osc_toggle_repo
            .as_ref()
            .ok_or_else(|| Error::Platform("No OSC toggle repository attached".to_string()))?;
        repo.create_trigger(trigger).await
    }
    
    async fn osc_update_trigger(&self, trigger: maowbot_common::models::osc_toggle::OscTrigger) -> Result<maowbot_common::models::osc_toggle::OscTrigger, Error> {
        let repo = self.osc_toggle_repo
            .as_ref()
            .ok_or_else(|| Error::Platform("No OSC toggle repository attached".to_string()))?;
        repo.update_trigger(trigger).await
    }
    
    async fn osc_delete_trigger(&self, trigger_id: i32) -> Result<(), Error> {
        let repo = self.osc_toggle_repo
            .as_ref()
            .ok_or_else(|| Error::Platform("No OSC toggle repository attached".to_string()))?;
        repo.delete_trigger(trigger_id).await
    }
    
    async fn osc_list_active_toggles(&self, user_id: Option<uuid::Uuid>) -> Result<Vec<maowbot_common::models::osc_toggle::OscToggleState>, Error> {
        let repo = self.osc_toggle_repo
            .as_ref()
            .ok_or_else(|| Error::Platform("No OSC toggle repository attached".to_string()))?;
        if let Some(uid) = user_id {
            repo.get_active_toggles(uid).await
        } else {
            repo.get_all_active_toggles().await
        }
    }
    
    async fn osc_activate_toggle(&self, redeem_id: uuid::Uuid, user_id: uuid::Uuid) -> Result<(), Error> {
        let osc_toggle_service = self.osc_toggle_service
            .as_ref()
            .ok_or_else(|| Error::Platform("No OSC toggle service attached".to_string()))?;
        
        osc_toggle_service.activate_toggle(redeem_id, user_id, None).await
    }
}
