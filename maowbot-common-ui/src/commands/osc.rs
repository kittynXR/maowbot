use crate::GrpcClient;
use super::CommandError;
use maowbot_proto::maowbot::services::{
    StartOscRequest, StopOscRequest, RestartOscRequest, GetOscStatusRequest,
    DiscoverPeersRequest, SendChatboxRequest, SendAvatarParameterRequest,
    ListTriggersWithRedeemsRequest, ListActiveTogglesRequest, OscConfig,
};
use maowbot_proto::maowbot::common::OscTrigger;

/// OSC service status
pub struct OscStatus {
    pub is_running: bool,
    pub listening_port: Option<i32>,
    pub is_oscquery_running: bool,
    pub oscquery_port: Option<i32>,
}

/// Result of trigger list operation
pub struct TriggerListResult {
    pub triggers: Vec<(SimplifiedOscTrigger, String)>, // (trigger, redeem_name)
}

/// Simplified OSC trigger for TUI display
pub struct SimplifiedOscTrigger {
    pub id: i32,
    pub redeem_id: String,
    pub parameter_name: String,
    pub parameter_type: String,
    pub on_value: String,
    pub off_value: String,
    pub duration_seconds: Option<i32>,
    pub cooldown_seconds: i32,
    pub enabled: bool,
}

/// Active toggle information
pub struct ActiveToggle {
    pub id: i32,
    pub trigger_id: i32,
    pub user_id: String,
    pub activated_at: String,
    pub expires_at: Option<String>,
}

/// OSC command handlers
pub struct OscCommands;

impl OscCommands {
    /// Start OSC service
    pub async fn start(client: &GrpcClient) -> Result<(), CommandError> {
        let request = StartOscRequest {
            config: Some(OscConfig {
                receive_port: 9001,
                send_port: 9000,
                bind_address: "127.0.0.1".to_string(),
                enable_oscquery: true,
                oscquery_port: 9002,
                auto_discover: true,
            }),
        };
        
        let mut osc_client = client.osc.clone();
        let response = osc_client
            .start_osc(request)
            .await
            .map_err(|e| CommandError::GrpcError(e.to_string()))?;
            
        let resp = response.into_inner();
        if !resp.success {
            return Err(CommandError::DataError(resp.error_message));
        }
        
        Ok(())
    }
    
    /// Stop OSC service
    pub async fn stop(client: &GrpcClient) -> Result<(), CommandError> {
        let request = StopOscRequest { force: false };
        
        let mut osc_client = client.osc.clone();
        osc_client
            .stop_osc(request)
            .await
            .map_err(|e| CommandError::GrpcError(e.to_string()))?;
            
        Ok(())
    }
    
    /// Restart OSC service
    pub async fn restart(client: &GrpcClient) -> Result<(), CommandError> {
        let request = RestartOscRequest { new_config: None };
        
        let mut osc_client = client.osc.clone();
        let response = osc_client
            .restart_osc(request)
            .await
            .map_err(|e| CommandError::GrpcError(e.to_string()))?;
            
        let resp = response.into_inner();
        if !resp.success {
            return Err(CommandError::DataError(resp.error_message));
        }
        
        Ok(())
    }
    
    /// Get OSC status
    pub async fn get_status(client: &GrpcClient) -> Result<OscStatus, CommandError> {
        let request = GetOscStatusRequest {};
        
        let mut osc_client = client.osc.clone();
        let response = osc_client
            .get_osc_status(request)
            .await
            .map_err(|e| CommandError::GrpcError(e.to_string()))?;
            
        let status = response.into_inner().status.unwrap_or_default();
        Ok(OscStatus {
            is_running: status.is_running,
            listening_port: status.config.as_ref().map(|c| c.receive_port),
            is_oscquery_running: status.config.as_ref().map(|c| c.enable_oscquery).unwrap_or(false),
            oscquery_port: status.config.as_ref().map(|c| c.oscquery_port),
        })
    }
    
    /// Discover OSCQuery peers
    pub async fn discover_peers(client: &GrpcClient) -> Result<Vec<String>, CommandError> {
        let request = DiscoverPeersRequest { timeout_seconds: 5 };
        
        let mut osc_client = client.osc.clone();
        let response = osc_client
            .discover_peers(request)
            .await
            .map_err(|e| CommandError::GrpcError(e.to_string()))?;
            
        let peers = response.into_inner().peers;
        Ok(peers.into_iter().map(|p| p.name).collect())
    }
    
    /// Send chatbox message
    pub async fn send_chatbox(client: &GrpcClient, message: &str) -> Result<(), CommandError> {
        let request = SendChatboxRequest {
            message: message.to_string(),
            notify_sound: false,
            use_typing_indicator: false,
        };
        
        let mut osc_client = client.osc.clone();
        osc_client
            .send_chatbox(request)
            .await
            .map_err(|e| CommandError::GrpcError(e.to_string()))?;
            
        Ok(())
    }
    
    /// Send avatar parameter
    pub async fn send_avatar_parameter_bool(
        client: &GrpcClient,
        param: &str,
        value: bool,
    ) -> Result<(), CommandError> {
        let request = SendAvatarParameterRequest {
            parameter_name: param.to_string(),
            value: Some(maowbot_proto::maowbot::services::send_avatar_parameter_request::Value::BoolValue(value)),
        };
        
        let mut osc_client = client.osc.clone();
        osc_client
            .send_avatar_parameter(request)
            .await
            .map_err(|e| CommandError::GrpcError(e.to_string()))?;
            
        Ok(())
    }
    
    pub async fn send_avatar_parameter_int(
        client: &GrpcClient,
        param: &str,
        value: i32,
    ) -> Result<(), CommandError> {
        let request = SendAvatarParameterRequest {
            parameter_name: param.to_string(),
            value: Some(maowbot_proto::maowbot::services::send_avatar_parameter_request::Value::IntValue(value)),
        };
        
        let mut osc_client = client.osc.clone();
        osc_client
            .send_avatar_parameter(request)
            .await
            .map_err(|e| CommandError::GrpcError(e.to_string()))?;
            
        Ok(())
    }
    
    pub async fn send_avatar_parameter_float(
        client: &GrpcClient,
        param: &str,
        value: f32,
    ) -> Result<(), CommandError> {
        let request = SendAvatarParameterRequest {
            parameter_name: param.to_string(),
            value: Some(maowbot_proto::maowbot::services::send_avatar_parameter_request::Value::FloatValue(value)),
        };
        
        let mut osc_client = client.osc.clone();
        osc_client
            .send_avatar_parameter(request)
            .await
            .map_err(|e| CommandError::GrpcError(e.to_string()))?;
            
        Ok(())
    }
    
    /// List triggers with redeem names
    pub async fn list_triggers_with_redeems(
        client: &GrpcClient,
    ) -> Result<TriggerListResult, CommandError> {
        let request = ListTriggersWithRedeemsRequest {};
        
        let mut osc_client = client.osc.clone();
        let response = osc_client
            .list_triggers_with_redeems(request)
            .await
            .map_err(|e| CommandError::GrpcError(e.to_string()))?;
            
        // Convert proto triggers to local triggers
        // Note: This is a simplified conversion - the actual service would need to handle this properly
        let triggers = response
            .into_inner()
            .triggers
            .into_iter()
            .filter_map(|t| {
                let trigger = t.trigger?;
                let redeem_name = t.linked_redeems.first()?.reward_name.clone();
                
                // Convert proto OscTrigger to simplified trigger
                // This is a simplified mapping - the actual proto should match the local model
                Some((
                    SimplifiedOscTrigger {
                        id: trigger.trigger_id,
                        redeem_id: String::new(), // Would need proper mapping
                        parameter_name: trigger.parameter_name,
                        parameter_type: "float".to_string(), // Would need proper mapping
                        on_value: trigger.max_value.to_string(),
                        off_value: trigger.min_value.to_string(),
                        duration_seconds: Some(trigger.hold_duration as i32),
                        cooldown_seconds: 0,
                        enabled: trigger.is_active,
                    },
                    redeem_name,
                ))
            })
            .collect();
            
        Ok(TriggerListResult { triggers })
    }
    
    /// List active toggles
    pub async fn list_active_toggles(
        client: &GrpcClient,
        user_id: Option<&str>,
    ) -> Result<Vec<ActiveToggle>, CommandError> {
        let request = ListActiveTogglesRequest {
            user_id: user_id.unwrap_or_default().to_string(),
        };
        
        let mut osc_client = client.osc.clone();
        let response = osc_client
            .list_active_toggles(request)
            .await
            .map_err(|e| CommandError::GrpcError(e.to_string()))?;
            
        let toggles = response
            .into_inner()
            .toggles
            .into_iter()
            .filter_map(|t| {
                Some(ActiveToggle {
                    id: t.toggle_id.parse().ok()?,
                    trigger_id: 0, // Would need proper mapping
                    user_id: t.user_id.clone(),
                    activated_at: t.activated_at.map(|ts| {
                        format!("{}", ts.seconds)
                    }).unwrap_or_else(|| "unknown".to_string()),
                    expires_at: t.expires_at.map(|ts| {
                        format!("{}", ts.seconds)
                    }),
                })
            })
            .collect();
            
        Ok(toggles)
    }
    
    // Note: Create, Update, Delete trigger operations would need proper proto message updates
    // to match the local OscTrigger model structure
}