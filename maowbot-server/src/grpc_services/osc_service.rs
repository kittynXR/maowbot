use tonic::{Request, Response, Status};
use maowbot_proto::maowbot::{
    common::OscPacket,
    services::{osc_service_server::OscService, *},
};
use maowbot_proto::maowbot::common;
use maowbot_core::plugins::manager::PluginManager;
use maowbot_common::traits::api::OscApi;
use maowbot_common::traits::osc_toggle_traits::OscToggleRepository;
use std::sync::Arc;
use chrono::Utc;
use tracing::{info, error, debug};
use prost_types;
use uuid::Uuid;

pub struct OscServiceImpl {
    plugin_manager: Arc<PluginManager>,
    osc_toggle_repo: Arc<dyn OscToggleRepository + Send + Sync>,
}

impl OscServiceImpl {
    pub fn new(plugin_manager: Arc<PluginManager>, osc_toggle_repo: Arc<dyn OscToggleRepository + Send + Sync>) -> Self {
        Self {
            plugin_manager,
            osc_toggle_repo,
        }
    }
}

#[tonic::async_trait]
impl OscService for OscServiceImpl {
    type StreamOSCPacketsStream = tonic::codec::Streaming<OscPacket>;
    type StreamOSCEventsStream = tonic::codec::Streaming<OscEvent>;
    async fn start_osc(&self, _: Request<StartOscRequest>) -> Result<Response<StartOscResponse>, Status> {
        info!("Starting OSC service");
        
        self.plugin_manager.osc_start().await
            .map_err(|e| Status::internal(format!("Failed to start OSC: {}", e)))?;
        
        // Get the status after starting
        let status = self.plugin_manager.osc_status().await
            .map_err(|e| Status::internal(format!("Failed to get OSC status: {}", e)))?;
        
        Ok(Response::new(StartOscResponse {
            success: true,
            error_message: String::new(),
            status: Some(OscStatus {
                is_running: status.is_running,
                config: Some(OscConfig {
                    receive_port: status.listening_port.unwrap_or(9001) as i32,
                    send_port: 9000, // Default
                    bind_address: "127.0.0.1".to_string(),
                    enable_oscquery: true,
                    oscquery_port: status.oscquery_port.unwrap_or(9002) as i32,
                    auto_discover: true,
                }),
                packets_sent: 0,
                packets_received: 0,
                started_at: Some(prost_types::Timestamp::from(std::time::SystemTime::now())),
                connected_peers: vec![],
                avatar_parameters: std::collections::HashMap::new(),
            }),
        }))
    }
    async fn stop_osc(&self, _: Request<StopOscRequest>) -> Result<Response<()>, Status> {
        info!("Stopping OSC service");
        
        self.plugin_manager.osc_stop().await
            .map_err(|e| Status::internal(format!("Failed to stop OSC: {}", e)))?;
        
        Ok(Response::new(()))
    }
    async fn restart_osc(&self, _: Request<RestartOscRequest>) -> Result<Response<RestartOscResponse>, Status> {
        info!("Restarting OSC service");
        
        // Stop first
        self.plugin_manager.osc_stop().await
            .map_err(|e| Status::internal(format!("Failed to stop OSC: {}", e)))?;
        
        // Then start
        self.plugin_manager.osc_start().await
            .map_err(|e| Status::internal(format!("Failed to start OSC: {}", e)))?;
        
        // Get the new status
        let status = self.plugin_manager.osc_status().await
            .map_err(|e| Status::internal(format!("Failed to get OSC status: {}", e)))?;
        
        Ok(Response::new(RestartOscResponse {
            success: true,
            error_message: String::new(),
            status: Some(OscStatus {
                is_running: status.is_running,
                config: Some(OscConfig {
                    receive_port: status.listening_port.unwrap_or(9001) as i32,
                    send_port: 9000, // Default
                    bind_address: "127.0.0.1".to_string(),
                    enable_oscquery: true,
                    oscquery_port: status.oscquery_port.unwrap_or(9002) as i32,
                    auto_discover: true,
                }),
                packets_sent: 0,
                packets_received: 0,
                started_at: Some(prost_types::Timestamp::from(std::time::SystemTime::now())),
                connected_peers: vec![],
                avatar_parameters: std::collections::HashMap::new(),
            }),
        }))
    }
    async fn get_osc_status(&self, _: Request<GetOscStatusRequest>) -> Result<Response<GetOscStatusResponse>, Status> {
        debug!("Getting OSC status");
        
        let status = self.plugin_manager.osc_status().await
            .map_err(|e| Status::internal(format!("Failed to get OSC status: {}", e)))?;
        
        let osc_status = OscStatus {
            is_running: status.is_running,
            config: Some(OscConfig {
                receive_port: status.listening_port.unwrap_or(9001) as i32,
                send_port: 9000, // Default
                bind_address: "127.0.0.1".to_string(),
                enable_oscquery: true,
                oscquery_port: status.oscquery_port.unwrap_or(9002) as i32,
                auto_discover: true,
            }),
            packets_sent: 0,
            packets_received: 0,
            started_at: Some(prost_types::Timestamp::from(std::time::SystemTime::now())),
            connected_peers: vec![],
            avatar_parameters: std::collections::HashMap::new(),
        };
        
        Ok(Response::new(GetOscStatusResponse {
            status: Some(osc_status),
        }))
    }
    async fn discover_peers(&self, _: Request<DiscoverPeersRequest>) -> Result<Response<DiscoverPeersResponse>, Status> {
        debug!("Discovering OSC peers");
        
        let peers = self.plugin_manager.osc_discover_peers().await
            .map_err(|e| Status::internal(format!("Failed to discover peers: {}", e)))?;
        
        // Convert to proto format
        let peer_infos: Vec<OscPeer> = peers.into_iter()
            .map(|addr| OscPeer {
                name: "Unknown".to_string(),
                address: addr,
                port: 9000, // Default OSC port
                service_type: "_oscjson._tcp".to_string(),
                properties: std::collections::HashMap::new(),
                discovered_at: Some(prost_types::Timestamp {
                    seconds: Utc::now().timestamp(),
                    nanos: 0,
                }),
            })
            .collect();
        
        Ok(Response::new(DiscoverPeersResponse {
            peers: peer_infos,
        }))
    }
    async fn get_peer_info(&self, request: Request<GetPeerInfoRequest>) -> Result<Response<GetPeerInfoResponse>, Status> {
        let req = request.into_inner();
        debug!("Getting OSC peer info for: {}:{}", req.peer_address, req.peer_port);
        
        // TODO: Implement peer info retrieval
        // For now, return mock data
        let peer = OscPeer {
            name: "VRChat".to_string(),
            address: req.peer_address.clone(),
            port: req.peer_port,
            service_type: "_oscjson._tcp".to_string(),
            properties: std::collections::HashMap::new(),
            discovered_at: Some(prost_types::Timestamp {
                seconds: Utc::now().timestamp(),
                nanos: 0,
            }),
        };
        
        Ok(Response::new(GetPeerInfoResponse {
            peer: Some(peer),
            nodes: vec![], // TODO: Populate OSC nodes
        }))
    }
    async fn send_chatbox(&self, request: Request<SendChatboxRequest>) -> Result<Response<()>, Status> {
        let req = request.into_inner();
        debug!("Sending OSC chatbox message: {}", req.message);
        
        self.plugin_manager.osc_chatbox(&req.message).await
            .map_err(|e| Status::internal(format!("Failed to send chatbox: {}", e)))?;
        
        Ok(Response::new(()))
    }
    async fn set_typing_indicator(&self, request: Request<SetTypingIndicatorRequest>) -> Result<Response<()>, Status> {
        let req = request.into_inner();
        debug!("Setting typing indicator: {}", req.is_typing);
        
        // Send typing indicator as a boolean parameter
        self.plugin_manager.osc_send_avatar_parameter_bool("Typing", req.is_typing).await
            .map_err(|e| Status::internal(format!("Failed to set typing indicator: {}", e)))?;
        
        Ok(Response::new(()))
    }
    async fn send_avatar_parameter(&self, request: Request<SendAvatarParameterRequest>) -> Result<Response<()>, Status> {
        let req = request.into_inner();
        debug!("Sending avatar parameter: {} = {:?}", req.parameter_name, req.value);
        
        if let Some(value) = req.value {
            match value {
                send_avatar_parameter_request::Value::BoolValue(b) => {
                    self.plugin_manager.osc_send_avatar_parameter_bool(&req.parameter_name, b).await
                        .map_err(|e| Status::internal(format!("Failed to send bool parameter: {}", e)))?;
                },
                send_avatar_parameter_request::Value::IntValue(i) => {
                    self.plugin_manager.osc_send_avatar_parameter_int(&req.parameter_name, i).await
                        .map_err(|e| Status::internal(format!("Failed to send int parameter: {}", e)))?;
                },
                send_avatar_parameter_request::Value::FloatValue(f) => {
                    self.plugin_manager.osc_send_avatar_parameter_float(&req.parameter_name, f).await
                        .map_err(|e| Status::internal(format!("Failed to send float parameter: {}", e)))?;
                },
            }
        }
        
        Ok(Response::new(()))
    }
    async fn batch_send_avatar_parameters(&self, request: Request<BatchSendAvatarParametersRequest>) -> Result<Response<BatchSendAvatarParametersResponse>, Status> {
        let req = request.into_inner();
        info!("Batch sending {} avatar parameters", req.parameters.len());
        
        let mut results: Vec<String> = Vec::new();
        
        let mut failed_params = Vec::new();
        let mut success_count = 0;
        
        for param in req.parameters {
            if let Some(value) = param.value {
                let send_result = match value {
                    avatar_parameter_update::Value::BoolValue(b) => {
                        self.plugin_manager.osc_send_avatar_parameter_bool(&param.parameter_name, b).await
                    },
                    avatar_parameter_update::Value::IntValue(i) => {
                        self.plugin_manager.osc_send_avatar_parameter_int(&param.parameter_name, i).await
                    },
                    avatar_parameter_update::Value::FloatValue(f) => {
                        self.plugin_manager.osc_send_avatar_parameter_float(&param.parameter_name, f).await
                    },
                };
                
                match send_result {
                    Ok(_) => {
                        success_count += 1;
                    },
                    Err(e) => {
                        debug!("Failed to send parameter {}: {}", param.parameter_name, e);
                        failed_params.push(param.parameter_name.clone());
                    }
                }
            } else {
                debug!("No value provided for parameter {}", param.parameter_name);
                failed_params.push(param.parameter_name.clone());
            }
        }
        
        Ok(Response::new(BatchSendAvatarParametersResponse {
            success_count,
            failed_parameters: failed_params,
        }))
    }
    async fn get_avatar_parameters(&self, _: Request<GetOscAvatarParametersRequest>) -> Result<Response<GetOscAvatarParametersResponse>, Status> {
        debug!("Getting avatar parameters");
        
        // TODO: Implement avatar parameter retrieval from OSCQuery
        // For now, return empty list
        Ok(Response::new(GetOscAvatarParametersResponse {
            parameters: vec![],
        }))
    }
    async fn send_input(&self, request: Request<SendInputRequest>) -> Result<Response<()>, Status> {
        let req = request.into_inner();
        debug!("Sending OSC input: {:?} = {}", req.input, req.value);
        
        // Send as a boolean parameter based on the input enum
        let input_name = match req.input {
            x if x == OscInput::Unknown as i32 => "Unknown",
            x if x == OscInput::Vertical as i32 => "Vertical",
            x if x == OscInput::Horizontal as i32 => "Horizontal",
            x if x == OscInput::MoveForward as i32 => "MoveForward",
            x if x == OscInput::MoveBackward as i32 => "MoveBackward",
            x if x == OscInput::MoveLeft as i32 => "MoveLeft",
            x if x == OscInput::MoveRight as i32 => "MoveRight",
            x if x == OscInput::LookLeft as i32 => "LookLeft",
            x if x == OscInput::LookRight as i32 => "LookRight",
            x if x == OscInput::Jump as i32 => "Jump",
            x if x == OscInput::Run as i32 => "Run",
            x if x == OscInput::Voice as i32 => "Voice",
            _ => "Unknown"
        };
        self.plugin_manager.osc_send_avatar_parameter_bool(&format!("Input/{}", input_name), req.value).await
            .map_err(|e| Status::internal(format!("Failed to send input: {}", e)))?;
        
        Ok(Response::new(()))
    }
    async fn send_axis_input(&self, request: Request<SendAxisInputRequest>) -> Result<Response<()>, Status> {
        let req = request.into_inner();
        debug!("Sending OSC axis input: {:?} = {}", req.axis, req.value);
        
        // Send as a float parameter based on the axis enum
        let axis_name = match req.axis {
            x if x == OscAxis::Unknown as i32 => "Unknown",
            x if x == OscAxis::Vertical as i32 => "Vertical",
            x if x == OscAxis::Horizontal as i32 => "Horizontal",
            x if x == OscAxis::LookHorizontal as i32 => "LookHorizontal",
            x if x == OscAxis::LookVertical as i32 => "LookVertical",
            _ => "Unknown"
        };
        self.plugin_manager.osc_send_avatar_parameter_float(&format!("Input/{}", axis_name), req.value).await
            .map_err(|e| Status::internal(format!("Failed to send axis input: {}", e)))?;
        
        Ok(Response::new(()))
    }
    async fn list_triggers(&self, _: Request<ListTriggersRequest>) -> Result<Response<ListTriggersResponse>, Status> {
        debug!("Listing OSC triggers");
        
        let triggers = self.plugin_manager.osc_list_triggers().await
            .map_err(|e| Status::internal(format!("Failed to list triggers: {}", e)))?;
        
        // Convert to proto format
        let trigger_protos: Vec<common::OscTrigger> = triggers.into_iter()
            .map(|t| common::OscTrigger {
                trigger_id: t.id,
                name: format!("trigger_{}", t.id),
                parameter_name: t.parameter_name,
                min_value: 0.0, // Using on_value as default
                max_value: 1.0, // Using off_value as default
                hold_duration: t.duration_seconds.unwrap_or(0) as f32,
                is_active: t.enabled,
                linked_redeems: vec![t.redeem_id.to_string()],
            })
            .collect();
        
        Ok(Response::new(ListTriggersResponse {
            triggers: trigger_protos,
        }))
    }
    async fn create_trigger(&self, request: Request<CreateTriggerRequest>) -> Result<Response<CreateTriggerResponse>, Status> {
        let req = request.into_inner();
        let trigger_proto = req.trigger.ok_or_else(|| Status::invalid_argument("Trigger is required"))?;
        info!("Creating OSC trigger: {}", trigger_proto.name);
        
        let redeem_id = if trigger_proto.linked_redeems.is_empty() {
            return Err(Status::invalid_argument("At least one linked redeem is required"));
        } else {
            Uuid::parse_str(&trigger_proto.linked_redeems[0])
                .map_err(|e| Status::invalid_argument(format!("Invalid redeem ID: {}", e)))?
        };
        
        let trigger = maowbot_common::models::osc_toggle::OscTrigger {
            id: 0, // Will be assigned by database
            redeem_id,
            parameter_name: trigger_proto.parameter_name.clone(),
            parameter_type: "float".to_string(), // Default to float
            on_value: trigger_proto.max_value.to_string(),
            off_value: trigger_proto.min_value.to_string(),
            duration_seconds: Some(trigger_proto.hold_duration as i32),
            cooldown_seconds: 0, // Default cooldown
            enabled: trigger_proto.is_active,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        
        let created = self.plugin_manager.osc_create_trigger(trigger).await
            .map_err(|e| Status::internal(format!("Failed to create trigger: {}", e)))?;
        
        // Convert to proto format
        let created_trigger_proto = common::OscTrigger {
            trigger_id: created.id,
            name: trigger_proto.name,
            parameter_name: created.parameter_name,
            min_value: trigger_proto.min_value,
            max_value: trigger_proto.max_value,
            hold_duration: created.duration_seconds.unwrap_or(0) as f32,
            is_active: created.enabled,
            linked_redeems: vec![created.redeem_id.to_string()],
        };
        
        Ok(Response::new(CreateTriggerResponse {
            trigger: Some(created_trigger_proto),
        }))
    }
    async fn update_trigger(&self, request: Request<UpdateTriggerRequest>) -> Result<Response<UpdateTriggerResponse>, Status> {
        let req = request.into_inner();
        let trigger_proto = req.trigger.ok_or_else(|| Status::invalid_argument("Trigger is required"))?;
        info!("Updating OSC trigger: {}", trigger_proto.trigger_id);
        
        // Get the existing trigger
        let existing = self.plugin_manager.osc_get_trigger(trigger_proto.trigger_id).await
            .map_err(|e| Status::internal(format!("Failed to get trigger: {}", e)))?;
        
        if let Some(mut trigger) = existing {
            // Update fields from proto
            if !trigger_proto.parameter_name.is_empty() {
                trigger.parameter_name = trigger_proto.parameter_name.clone();
            }
            trigger.on_value = trigger_proto.max_value.to_string();
            trigger.off_value = trigger_proto.min_value.to_string();
            trigger.duration_seconds = Some(trigger_proto.hold_duration as i32);
            trigger.enabled = trigger_proto.is_active;
            trigger.updated_at = Utc::now();
            
            let updated = self.plugin_manager.osc_update_trigger(trigger).await
                .map_err(|e| Status::internal(format!("Failed to update trigger: {}", e)))?;
            
            // Convert to proto format
            let updated_trigger_proto = common::OscTrigger {
                trigger_id: updated.id,
                name: trigger_proto.name,
                parameter_name: updated.parameter_name,
                min_value: trigger_proto.min_value,
                max_value: trigger_proto.max_value,
                hold_duration: updated.duration_seconds.unwrap_or(0) as f32,
                is_active: updated.enabled,
                linked_redeems: vec![updated.redeem_id.to_string()],
            };
            
            Ok(Response::new(UpdateTriggerResponse {
                trigger: Some(updated_trigger_proto),
            }))
        } else {
            Err(Status::not_found("Trigger not found"))
        }
    }
    async fn delete_trigger(&self, request: Request<DeleteTriggerRequest>) -> Result<Response<()>, Status> {
        let req = request.into_inner();
        info!("Deleting OSC trigger: {}", req.trigger_id);
        
        self.plugin_manager.osc_delete_trigger(req.trigger_id).await
            .map_err(|e| Status::internal(format!("Failed to delete trigger: {}", e)))?;
        
        Ok(Response::new(()))
    }
    async fn list_triggers_with_redeems(&self, _: Request<ListTriggersWithRedeemsRequest>) -> Result<Response<ListTriggersWithRedeemsResponse>, Status> {
        debug!("Listing OSC triggers with redeem names");
        
        let triggers_with_names = self.plugin_manager.osc_list_triggers_with_redeems().await
            .map_err(|e| Status::internal(format!("Failed to list triggers with redeems: {}", e)))?;
        
        // Convert to proto format
        // Convert triggers with redeem names to TriggerWithRedeems structure
        let mut trigger_redeems_map: std::collections::HashMap<i32, (common::OscTrigger, Vec<String>)> = std::collections::HashMap::new();
        
        for (trigger, redeem_name) in triggers_with_names {
            let trigger_proto = common::OscTrigger {
                trigger_id: trigger.id,
                name: format!("trigger_{}", trigger.id),
                parameter_name: trigger.parameter_name,
                min_value: 0.0,
                max_value: 1.0,
                hold_duration: trigger.duration_seconds.unwrap_or(0) as f32,
                is_active: trigger.enabled,
                linked_redeems: vec![trigger.redeem_id.to_string()],
            };
            
            trigger_redeems_map.insert(trigger.id, (trigger_proto, vec![redeem_name]));
        }
        
        let triggers: Vec<TriggerWithRedeems> = trigger_redeems_map.into_iter()
            .map(|(_, (trigger, redeem_names))| TriggerWithRedeems {
                trigger: Some(trigger),
                linked_redeems: vec![], // TODO: Convert redeem names to full Redeem protos
            })
            .collect();
        
        Ok(Response::new(ListTriggersWithRedeemsResponse {
            triggers,
        }))
    }
    async fn list_active_toggles(&self, request: Request<ListActiveTogglesRequest>) -> Result<Response<ListActiveTogglesResponse>, Status> {
        let req = request.into_inner();
        debug!("Listing active OSC toggles");
        
        let user_id = if req.user_id.is_empty() {
            None
        } else {
            Some(Uuid::parse_str(&req.user_id)
                .map_err(|e| Status::invalid_argument(format!("Invalid user ID: {}", e)))?)
        };
        
        let toggle_states = self.plugin_manager.osc_list_active_toggles(user_id).await
            .map_err(|e| Status::internal(format!("Failed to list active toggles: {}", e)))?;
        
        // Convert to proto format
        let toggles: Vec<ActiveToggle> = toggle_states.into_iter()
            .map(|state| ActiveToggle {
                toggle_id: state.id.to_string(),
                user_id: state.user_id.to_string(),
                parameter_name: String::new(), // TODO: Look up from trigger
                current_state: state.is_active,
                activated_at: Some(prost_types::Timestamp {
                    seconds: state.activated_at.timestamp(),
                    nanos: state.activated_at.timestamp_subsec_nanos() as i32,
                }),
                expires_at: state.expires_at.map(|ts| prost_types::Timestamp {
                    seconds: ts.timestamp(),
                    nanos: ts.timestamp_subsec_nanos() as i32,
                }),
            })
            .collect();
        
        Ok(Response::new(ListActiveTogglesResponse {
            toggles,
        }))
    }
    async fn set_toggle_state(&self, request: Request<SetToggleStateRequest>) -> Result<Response<()>, Status> {
        let req = request.into_inner();
        info!("Setting toggle state: {} = {}", req.toggle_id, req.state);
        
        // For toggle state, we need to look up the toggle to get the associated redeem and user
        // TODO: This needs proper implementation
        let toggle_id = req.toggle_id.parse::<i32>()
            .map_err(|e| Status::invalid_argument(format!("Invalid toggle ID: {}", e)))?;
        
        // TODO: Implement toggle state management
        // This requires looking up the toggle and updating its state
        return Err(Status::unimplemented("Toggle state management not yet implemented"));
        
        Ok(Response::new(()))
    }
    async fn send_raw_osc(&self, request: Request<SendRawOscRequest>) -> Result<Response<()>, Status> {
        let req = request.into_inner();
        info!("Sending raw OSC to address: {}", req.address);
        
        // TODO: Implement raw OSC sending
        // This would require parsing the args and constructing an OSC message
        Err(Status::unimplemented("Raw OSC sending not yet implemented"))
    }
    async fn stream_osc_packets(&self, _: Request<StreamOscPacketsRequest>) -> Result<Response<Self::StreamOSCPacketsStream>, Status> {
        Err(Status::unimplemented("Not implemented"))
    }
    async fn stream_osc_events(&self, _: Request<StreamOscEventsRequest>) -> Result<Response<Self::StreamOSCEventsStream>, Status> {
        Err(Status::unimplemented("Not implemented"))
    }
}