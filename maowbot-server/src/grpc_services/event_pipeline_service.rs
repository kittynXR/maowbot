use tonic::{Request, Response, Status};
use std::sync::Arc;
use tracing::{info, error, debug};
use maowbot_proto::maowbot::services::event_pipeline::event_pipeline_service_server::EventPipelineService as GrpcEventPipelineService;
use maowbot_proto::maowbot::services::event_pipeline::*;
use maowbot_common::traits::event_pipeline_traits::{
    EventPipelineRepository, PipelineExecutionLogRepository,
};
use maowbot_common::models::event_pipeline::{
    EventPipeline as DbPipeline, PipelineFilter as DbFilter, PipelineAction as DbAction,
};
use uuid::Uuid;
use chrono::Utc;

use crate::context::ServerContext;

pub struct EventPipelineServiceImpl {
    ctx: Arc<ServerContext>,
}

impl EventPipelineServiceImpl {
    pub fn new(ctx: Arc<ServerContext>) -> Self {
        Self { ctx }
    }
    
    fn db_pipeline_to_proto(pipeline: &DbPipeline) -> Pipeline {
        Pipeline {
            pipeline_id: pipeline.pipeline_id.to_string(),
            name: pipeline.name.clone(),
            description: pipeline.description.clone().unwrap_or_default(),
            enabled: pipeline.enabled,
            priority: pipeline.priority,
            stop_on_match: pipeline.stop_on_match,
            stop_on_error: pipeline.stop_on_error,
            is_system: pipeline.is_system,
            tags: pipeline.tags.clone(),
            metadata: pipeline.metadata.to_string(),
            execution_count: pipeline.execution_count,
            success_count: pipeline.success_count,
            last_executed: pipeline.last_executed.map(|dt| dt.to_rfc3339()).unwrap_or_default(),
            created_at: pipeline.created_at.to_rfc3339(),
            updated_at: pipeline.updated_at.to_rfc3339(),
        }
    }
    
    fn db_filter_to_proto(filter: &DbFilter) -> PipelineFilter {
        PipelineFilter {
            filter_id: filter.filter_id.to_string(),
            pipeline_id: filter.pipeline_id.to_string(),
            filter_type: filter.filter_type.clone(),
            filter_config: filter.filter_config.to_string(),
            filter_order: filter.filter_order,
            is_negated: filter.is_negated,
            is_required: filter.is_required,
            created_at: filter.created_at.to_rfc3339(),
            updated_at: filter.updated_at.to_rfc3339(),
        }
    }
    
    fn db_action_to_proto(action: &DbAction) -> PipelineAction {
        PipelineAction {
            action_id: action.action_id.to_string(),
            pipeline_id: action.pipeline_id.to_string(),
            action_type: action.action_type.clone(),
            action_config: action.action_config.to_string(),
            action_order: action.action_order,
            continue_on_error: action.continue_on_error,
            is_async: action.is_async,
            timeout_ms: action.timeout_ms,
            retry_count: action.retry_count,
            retry_delay_ms: action.retry_delay_ms,
            created_at: action.created_at.to_rfc3339(),
            updated_at: action.updated_at.to_rfc3339(),
        }
    }
}

#[tonic::async_trait]
impl GrpcEventPipelineService for EventPipelineServiceImpl {
    async fn create_pipeline(
        &self,
        request: Request<CreatePipelineRequest>,
    ) -> Result<Response<CreatePipelineResponse>, Status> {
        let req = request.into_inner();
        debug!("Creating pipeline: {}", req.name);
        
        use maowbot_common::models::event_pipeline::CreatePipelineRequest as DbCreatePipelineRequest;
        
        let create_request = DbCreatePipelineRequest {
            name: req.name,
            description: if req.description.is_empty() { None } else { Some(req.description) },
            enabled: true,
            priority: req.priority,
            stop_on_match: req.stop_on_match,
            stop_on_error: req.stop_on_error,
            tags: req.tags,
            metadata: Some(serde_json::json!({})),
        };
        
        match self.ctx.event_pipeline_service.repository.create_pipeline(&create_request).await {
            Ok(created) => {
                // Reload pipelines in the service
                let _ = self.ctx.event_pipeline_service.reload_pipelines().await;
                
                Ok(Response::new(CreatePipelineResponse {
                    success: true,
                    message: format!("Pipeline '{}' created successfully", created.name),
                    pipeline: Some(Self::db_pipeline_to_proto(&created)),
                }))
            }
            Err(e) => {
                error!("Failed to create pipeline: {:?}", e);
                Ok(Response::new(CreatePipelineResponse {
                    success: false,
                    message: format!("Failed to create pipeline: {}", e),
                    pipeline: None,
                }))
            }
        }
    }
    
    async fn update_pipeline(
        &self,
        request: Request<UpdatePipelineRequest>,
    ) -> Result<Response<UpdatePipelineResponse>, Status> {
        let req = request.into_inner();
        debug!("Updating pipeline: {}", req.pipeline_id);
        
        // Parse pipeline ID
        let pipeline_id = match Uuid::parse_str(&req.pipeline_id) {
            Ok(id) => id,
            Err(e) => {
                return Ok(Response::new(UpdatePipelineResponse {
                    success: false,
                    message: format!("Invalid pipeline ID: {}", e),
                    pipeline: None,
                }));
            }
        };
        
        // Get existing pipeline
        let existing = match self.ctx.event_pipeline_service.repository.get_pipeline(pipeline_id).await {
            Ok(Some(p)) => p,
            Ok(None) => {
                return Ok(Response::new(UpdatePipelineResponse {
                    success: false,
                    message: format!("Pipeline with ID {} not found", req.pipeline_id),
                    pipeline: None,
                }));
            }
            Err(e) => {
                return Ok(Response::new(UpdatePipelineResponse {
                    success: false,
                    message: format!("Failed to get pipeline: {}", e),
                    pipeline: None,
                }));
            }
        };
        
        use maowbot_common::models::event_pipeline::UpdatePipelineRequest as DbUpdatePipelineRequest;
        
        let update_request = DbUpdatePipelineRequest {
            name: req.name,
            description: req.description,
            enabled: req.enabled,
            priority: req.priority,
            stop_on_match: req.stop_on_match,
            stop_on_error: req.stop_on_error,
            tags: None, // Not updating tags for now
            metadata: None, // Not updating metadata for now
        };
        
        match self.ctx.event_pipeline_service.repository.update_pipeline(pipeline_id, &update_request).await {
            Ok(updated) => {
                // Reload pipelines in the service
                let _ = self.ctx.event_pipeline_service.reload_pipelines().await;
                
                Ok(Response::new(UpdatePipelineResponse {
                    success: true,
                    message: format!("Pipeline '{}' updated successfully", updated.name),
                    pipeline: Some(Self::db_pipeline_to_proto(&updated)),
                }))
            }
            Err(e) => {
                error!("Failed to update pipeline: {:?}", e);
                Ok(Response::new(UpdatePipelineResponse {
                    success: false,
                    message: format!("Failed to update pipeline: {}", e),
                    pipeline: None,
                }))
            }
        }
    }
    
    async fn delete_pipeline(
        &self,
        request: Request<DeletePipelineRequest>,
    ) -> Result<Response<DeletePipelineResponse>, Status> {
        let req = request.into_inner();
        debug!("Deleting pipeline: {}", req.pipeline_id);
        
        // Parse pipeline ID
        let pipeline_id = match Uuid::parse_str(&req.pipeline_id) {
            Ok(id) => id,
            Err(e) => {
                return Ok(Response::new(DeletePipelineResponse {
                    success: false,
                    message: format!("Invalid pipeline ID: {}", e),
                }));
            }
        };
        
        match self.ctx.event_pipeline_service.repository.delete_pipeline(pipeline_id).await {
            Ok(_) => {
                // Reload pipelines in the service
                let _ = self.ctx.event_pipeline_service.reload_pipelines().await;
                
                Ok(Response::new(DeletePipelineResponse {
                    success: true,
                    message: format!("Pipeline {} deleted successfully", req.pipeline_id),
                }))
            }
            Err(e) => {
                error!("Failed to delete pipeline: {:?}", e);
                Ok(Response::new(DeletePipelineResponse {
                    success: false,
                    message: format!("Failed to delete pipeline: {}", e),
                }))
            }
        }
    }
    
    async fn get_pipeline(
        &self,
        request: Request<GetPipelineRequest>,
    ) -> Result<Response<GetPipelineResponse>, Status> {
        let req = request.into_inner();
        debug!("Getting pipeline: {}", req.pipeline_id);
        
        // Parse pipeline ID
        let pipeline_id = match Uuid::parse_str(&req.pipeline_id) {
            Ok(id) => id,
            Err(e) => {
                return Ok(Response::new(GetPipelineResponse {
                    success: false,
                    message: format!("Invalid pipeline ID: {}", e),
                    pipeline: None,
                }));
            }
        };
        
        match self.ctx.event_pipeline_service.repository.get_pipeline(pipeline_id).await {
            Ok(Some(pipeline)) => {
                Ok(Response::new(GetPipelineResponse {
                    success: true,
                    message: "Pipeline retrieved successfully".to_string(),
                    pipeline: Some(Self::db_pipeline_to_proto(&pipeline)),
                }))
            }
            Ok(None) => {
                Ok(Response::new(GetPipelineResponse {
                    success: false,
                    message: format!("Pipeline with ID {} not found", req.pipeline_id),
                    pipeline: None,
                }))
            }
            Err(e) => {
                error!("Failed to get pipeline: {:?}", e);
                Ok(Response::new(GetPipelineResponse {
                    success: false,
                    message: format!("Failed to get pipeline: {}", e),
                    pipeline: None,
                }))
            }
        }
    }
    
    async fn list_pipelines(
        &self,
        request: Request<ListPipelinesRequest>,
    ) -> Result<Response<ListPipelinesResponse>, Status> {
        let req = request.into_inner();
        debug!("Listing pipelines (include_disabled: {})", req.include_disabled);
        
        match self.ctx.event_pipeline_service.repository.list_pipelines(!req.include_disabled).await {
            Ok(pipelines) => {
                let proto_pipelines: Vec<Pipeline> = pipelines
                    .iter()
                    .map(Self::db_pipeline_to_proto)
                    .collect();
                
                Ok(Response::new(ListPipelinesResponse {
                    success: true,
                    message: format!("Found {} pipelines", proto_pipelines.len()),
                    pipelines: proto_pipelines,
                }))
            }
            Err(e) => {
                error!("Failed to list pipelines: {:?}", e);
                Ok(Response::new(ListPipelinesResponse {
                    success: false,
                    message: format!("Failed to list pipelines: {}", e),
                    pipelines: vec![],
                }))
            }
        }
    }
    
    async fn toggle_pipeline(
        &self,
        request: Request<TogglePipelineRequest>,
    ) -> Result<Response<TogglePipelineResponse>, Status> {
        let req = request.into_inner();
        debug!("Toggling pipeline {} to enabled={}", req.pipeline_id, req.enabled);
        
        // Parse pipeline ID
        let pipeline_id = match Uuid::parse_str(&req.pipeline_id) {
            Ok(id) => id,
            Err(e) => {
                return Ok(Response::new(TogglePipelineResponse {
                    success: false,
                    message: format!("Invalid pipeline ID: {}", e),
                }));
            }
        };
        
        use maowbot_common::models::event_pipeline::UpdatePipelineRequest as DbUpdatePipelineRequest;
        
        let update_request = DbUpdatePipelineRequest {
            name: None,
            description: None,
            enabled: Some(req.enabled),
            priority: None,
            stop_on_match: None,
            stop_on_error: None,
            tags: None,
            metadata: None,
        };
        
        match self.ctx.event_pipeline_service.repository.update_pipeline(pipeline_id, &update_request).await {
            Ok(_) => {
                // Reload pipelines in the service
                let _ = self.ctx.event_pipeline_service.reload_pipelines().await;
                
                Ok(Response::new(TogglePipelineResponse {
                    success: true,
                    message: format!("Pipeline {} {} successfully", 
                        req.pipeline_id, 
                        if req.enabled { "enabled" } else { "disabled" }
                    ),
                }))
            }
            Err(e) => {
                error!("Failed to toggle pipeline: {:?}", e);
                Ok(Response::new(TogglePipelineResponse {
                    success: false,
                    message: format!("Failed to toggle pipeline: {}", e),
                }))
            }
        }
    }
    
    async fn add_filter(
        &self,
        request: Request<AddFilterRequest>,
    ) -> Result<Response<AddFilterResponse>, Status> {
        let req = request.into_inner();
        debug!("Adding filter to pipeline {}: {}", req.pipeline_id, req.filter_type);
        
        // Parse pipeline ID
        let pipeline_id = match Uuid::parse_str(&req.pipeline_id) {
            Ok(id) => id,
            Err(e) => {
                return Ok(Response::new(AddFilterResponse {
                    success: false,
                    message: format!("Invalid pipeline ID: {}", e),
                    filter: None,
                }));
            }
        };
        
        let filter_config: serde_json::Value = match serde_json::from_str(&req.filter_config) {
            Ok(v) => v,
            Err(e) => {
                return Ok(Response::new(AddFilterResponse {
                    success: false,
                    message: format!("Invalid filter configuration JSON: {}", e),
                    filter: None,
                }));
            }
        };
        
        use maowbot_common::models::event_pipeline::CreateFilterRequest as DbCreateFilterRequest;
        
        let filter_request = DbCreateFilterRequest {
            filter_type: req.filter_type,
            filter_config,
            filter_order: req.filter_order.unwrap_or(999),
            is_negated: req.is_negated,
            is_required: req.is_required,
        };
        
        match self.ctx.event_pipeline_service.repository.add_filter(pipeline_id, &filter_request).await {
            Ok(created) => {
                // Reload pipelines in the service
                let _ = self.ctx.event_pipeline_service.reload_pipelines().await;
                
                Ok(Response::new(AddFilterResponse {
                    success: true,
                    message: "Filter added successfully".to_string(),
                    filter: Some(Self::db_filter_to_proto(&created)),
                }))
            }
            Err(e) => {
                error!("Failed to add filter: {:?}", e);
                Ok(Response::new(AddFilterResponse {
                    success: false,
                    message: format!("Failed to add filter: {}", e),
                    filter: None,
                }))
            }
        }
    }
    
    async fn update_filter(
        &self,
        request: Request<UpdateFilterRequest>,
    ) -> Result<Response<UpdateFilterResponse>, Status> {
        let req = request.into_inner();
        debug!("Updating filter: {}", req.filter_id);
        
        // Parse filter ID
        let filter_id = match Uuid::parse_str(&req.filter_id) {
            Ok(id) => id,
            Err(e) => {
                return Ok(Response::new(UpdateFilterResponse {
                    success: false,
                    message: format!("Invalid filter ID: {}", e),
                    filter: None,
                }));
            }
        };
        
        // Get existing filter
        let existing = match self.ctx.event_pipeline_service.repository.get_filter(filter_id).await {
            Ok(Some(f)) => f,
            Ok(None) => {
                return Ok(Response::new(UpdateFilterResponse {
                    success: false,
                    message: format!("Filter with ID {} not found", req.filter_id),
                    filter: None,
                }));
            }
            Err(e) => {
                return Ok(Response::new(UpdateFilterResponse {
                    success: false,
                    message: format!("Failed to get filter: {}", e),
                    filter: None,
                }));
            }
        };
        
        use maowbot_common::models::event_pipeline::CreateFilterRequest as DbCreateFilterRequest;
        
        let filter_request = DbCreateFilterRequest {
            filter_type: existing.filter_type.clone(),
            filter_config: if let Some(config_str) = req.filter_config {
                serde_json::from_str(&config_str).map_err(|e| Status::invalid_argument(format!("Invalid filter configuration JSON: {}", e)))?
            } else {
                existing.filter_config.clone()
            },
            filter_order: req.filter_order.unwrap_or(existing.filter_order),
            is_negated: req.is_negated.unwrap_or(existing.is_negated),
            is_required: req.is_required.unwrap_or(existing.is_required),
        };
        
        match self.ctx.event_pipeline_service.repository.update_filter(filter_id, &filter_request).await {
            Ok(updated) => {
                // Reload pipelines in the service
                let _ = self.ctx.event_pipeline_service.reload_pipelines().await;
                
                Ok(Response::new(UpdateFilterResponse {
                    success: true,
                    message: "Filter updated successfully".to_string(),
                    filter: Some(Self::db_filter_to_proto(&updated)),
                }))
            }
            Err(e) => {
                error!("Failed to update filter: {:?}", e);
                Ok(Response::new(UpdateFilterResponse {
                    success: false,
                    message: format!("Failed to update filter: {}", e),
                    filter: None,
                }))
            }
        }
    }
    
    async fn remove_filter(
        &self,
        request: Request<RemoveFilterRequest>,
    ) -> Result<Response<RemoveFilterResponse>, Status> {
        let req = request.into_inner();
        debug!("Removing filter: {}", req.filter_id);
        
        // Parse filter ID
        let filter_id = match Uuid::parse_str(&req.filter_id) {
            Ok(id) => id,
            Err(e) => {
                return Ok(Response::new(RemoveFilterResponse {
                    success: false,
                    message: format!("Invalid filter ID: {}", e),
                }));
            }
        };
        
        match self.ctx.event_pipeline_service.repository.delete_filter(filter_id).await {
            Ok(_) => {
                // Reload pipelines in the service
                let _ = self.ctx.event_pipeline_service.reload_pipelines().await;
                
                Ok(Response::new(RemoveFilterResponse {
                    success: true,
                    message: format!("Filter {} removed successfully", req.filter_id),
                }))
            }
            Err(e) => {
                error!("Failed to remove filter: {:?}", e);
                Ok(Response::new(RemoveFilterResponse {
                    success: false,
                    message: format!("Failed to remove filter: {}", e),
                }))
            }
        }
    }
    
    async fn list_filters(
        &self,
        request: Request<ListFiltersRequest>,
    ) -> Result<Response<ListFiltersResponse>, Status> {
        let req = request.into_inner();
        debug!("Listing filters for pipeline: {}", req.pipeline_id);
        
        // Parse pipeline ID
        let pipeline_id = match Uuid::parse_str(&req.pipeline_id) {
            Ok(id) => id,
            Err(e) => {
                return Ok(Response::new(ListFiltersResponse {
                    success: false,
                    message: format!("Invalid pipeline ID: {}", e),
                    filters: vec![],
                }));
            }
        };
        
        match self.ctx.event_pipeline_service.repository.list_filters_for_pipeline(pipeline_id).await {
            Ok(filters) => {
                let proto_filters: Vec<PipelineFilter> = filters
                    .iter()
                    .map(Self::db_filter_to_proto)
                    .collect();
                
                Ok(Response::new(ListFiltersResponse {
                    success: true,
                    message: format!("Found {} filters", proto_filters.len()),
                    filters: proto_filters,
                }))
            }
            Err(e) => {
                error!("Failed to list filters: {:?}", e);
                Ok(Response::new(ListFiltersResponse {
                    success: false,
                    message: format!("Failed to list filters: {}", e),
                    filters: vec![],
                }))
            }
        }
    }
    
    async fn add_action(
        &self,
        request: Request<AddActionRequest>,
    ) -> Result<Response<AddActionResponse>, Status> {
        let req = request.into_inner();
        debug!("Adding action to pipeline {}: {}", req.pipeline_id, req.action_type);
        
        let action_config: serde_json::Value = match serde_json::from_str(&req.action_config) {
            Ok(v) => v,
            Err(e) => {
                return Ok(Response::new(AddActionResponse {
                    success: false,
                    message: format!("Invalid action configuration JSON: {}", e),
                    action: None,
                }));
            }
        };
        
        // Parse pipeline ID
        let pipeline_id = match Uuid::parse_str(&req.pipeline_id) {
            Ok(id) => id,
            Err(e) => {
                return Ok(Response::new(AddActionResponse {
                    success: false,
                    message: format!("Invalid pipeline ID: {}", e),
                    action: None,
                }));
            }
        };
        
        use maowbot_common::models::event_pipeline::CreateActionRequest as DbCreateActionRequest;
        
        let action_request = DbCreateActionRequest {
            action_type: req.action_type,
            action_config,
            action_order: req.action_order.unwrap_or(999),
            continue_on_error: req.continue_on_error,
            is_async: req.is_async,
            timeout_ms: req.timeout_ms,
            retry_count: req.retry_count,
            retry_delay_ms: req.retry_delay_ms,
            condition_type: None,
            condition_config: None,
        };
        
        match self.ctx.event_pipeline_service.repository.add_action(pipeline_id, &action_request).await {
            Ok(created) => {
                // Reload pipelines in the service
                let _ = self.ctx.event_pipeline_service.reload_pipelines().await;
                
                Ok(Response::new(AddActionResponse {
                    success: true,
                    message: "Action added successfully".to_string(),
                    action: Some(Self::db_action_to_proto(&created)),
                }))
            }
            Err(e) => {
                error!("Failed to add action: {:?}", e);
                Ok(Response::new(AddActionResponse {
                    success: false,
                    message: format!("Failed to add action: {}", e),
                    action: None,
                }))
            }
        }
    }
    
    async fn update_action(
        &self,
        request: Request<UpdateActionRequest>,
    ) -> Result<Response<UpdateActionResponse>, Status> {
        let req = request.into_inner();
        debug!("Updating action: {}", req.action_id);
        
        // Parse action ID
        let action_id = match Uuid::parse_str(&req.action_id) {
            Ok(id) => id,
            Err(e) => {
                return Ok(Response::new(UpdateActionResponse {
                    success: false,
                    message: format!("Invalid action ID: {}", e),
                    action: None,
                }));
            }
        };
        
        // Get existing action
        let existing = match self.ctx.event_pipeline_service.repository.get_action(action_id).await {
            Ok(Some(a)) => a,
            Ok(None) => {
                return Ok(Response::new(UpdateActionResponse {
                    success: false,
                    message: format!("Action with ID {} not found", req.action_id),
                    action: None,
                }));
            }
            Err(e) => {
                return Ok(Response::new(UpdateActionResponse {
                    success: false,
                    message: format!("Failed to get action: {}", e),
                    action: None,
                }));
            }
        };
        
        use maowbot_common::models::event_pipeline::CreateActionRequest as DbCreateActionRequest;
        
        let action_request = DbCreateActionRequest {
            action_type: existing.action_type.clone(),
            action_config: if let Some(config_str) = req.action_config {
                serde_json::from_str(&config_str).map_err(|e| Status::invalid_argument(format!("Invalid action configuration JSON: {}", e)))?
            } else {
                existing.action_config.clone()
            },
            action_order: req.action_order.unwrap_or(existing.action_order),
            continue_on_error: req.continue_on_error.unwrap_or(existing.continue_on_error),
            is_async: req.is_async.unwrap_or(existing.is_async),
            timeout_ms: req.timeout_ms.or(existing.timeout_ms),
            retry_count: req.retry_count.unwrap_or(existing.retry_count),
            retry_delay_ms: req.retry_delay_ms.unwrap_or(existing.retry_delay_ms),
            condition_type: existing.condition_type.clone(),
            condition_config: existing.condition_config.clone(),
        };
        
        match self.ctx.event_pipeline_service.repository.update_action(action_id, &action_request).await {
            Ok(updated) => {
                // Reload pipelines in the service
                let _ = self.ctx.event_pipeline_service.reload_pipelines().await;
                
                Ok(Response::new(UpdateActionResponse {
                    success: true,
                    message: "Action updated successfully".to_string(),
                    action: Some(Self::db_action_to_proto(&updated)),
                }))
            }
            Err(e) => {
                error!("Failed to update action: {:?}", e);
                Ok(Response::new(UpdateActionResponse {
                    success: false,
                    message: format!("Failed to update action: {}", e),
                    action: None,
                }))
            }
        }
    }
    
    async fn remove_action(
        &self,
        request: Request<RemoveActionRequest>,
    ) -> Result<Response<RemoveActionResponse>, Status> {
        let req = request.into_inner();
        debug!("Removing action: {}", req.action_id);
        
        // Parse action ID
        let action_id = match Uuid::parse_str(&req.action_id) {
            Ok(id) => id,
            Err(e) => {
                return Ok(Response::new(RemoveActionResponse {
                    success: false,
                    message: format!("Invalid action ID: {}", e),
                }));
            }
        };
        
        match self.ctx.event_pipeline_service.repository.delete_action(action_id).await {
            Ok(_) => {
                // Reload pipelines in the service
                let _ = self.ctx.event_pipeline_service.reload_pipelines().await;
                
                Ok(Response::new(RemoveActionResponse {
                    success: true,
                    message: format!("Action {} removed successfully", req.action_id),
                }))
            }
            Err(e) => {
                error!("Failed to remove action: {:?}", e);
                Ok(Response::new(RemoveActionResponse {
                    success: false,
                    message: format!("Failed to remove action: {}", e),
                }))
            }
        }
    }
    
    async fn list_actions(
        &self,
        request: Request<ListActionsRequest>,
    ) -> Result<Response<ListActionsResponse>, Status> {
        let req = request.into_inner();
        debug!("Listing actions for pipeline: {}", req.pipeline_id);
        
        // Parse pipeline ID
        let pipeline_id = match Uuid::parse_str(&req.pipeline_id) {
            Ok(id) => id,
            Err(e) => {
                return Ok(Response::new(ListActionsResponse {
                    success: false,
                    message: format!("Invalid pipeline ID: {}", e),
                    actions: vec![],
                }));
            }
        };
        
        match self.ctx.event_pipeline_service.repository.list_actions_for_pipeline(pipeline_id).await {
            Ok(actions) => {
                let proto_actions: Vec<PipelineAction> = actions
                    .iter()
                    .map(Self::db_action_to_proto)
                    .collect();
                
                Ok(Response::new(ListActionsResponse {
                    success: true,
                    message: format!("Found {} actions", proto_actions.len()),
                    actions: proto_actions,
                }))
            }
            Err(e) => {
                error!("Failed to list actions: {:?}", e);
                Ok(Response::new(ListActionsResponse {
                    success: false,
                    message: format!("Failed to list actions: {}", e),
                    actions: vec![],
                }))
            }
        }
    }
    
    async fn get_available_filters(
        &self,
        _request: Request<GetAvailableFiltersRequest>,
    ) -> Result<Response<GetAvailableFiltersResponse>, Status> {
        debug!("Getting available filter types");
        
        // TODO: Get these from the EventPipelineService registry
        let filters = vec![
            FilterType {
                id: "platform_filter".to_string(),
                name: "Platform Filter".to_string(),
                description: "Filter events by platform (Twitch, Discord, etc.)".to_string(),
                config_schema: r#"{"type":"object","properties":{"platforms":{"type":"array","items":{"type":"string"}}}}"#.to_string(),
            },
            FilterType {
                id: "channel_filter".to_string(),
                name: "Channel Filter".to_string(),
                description: "Filter events by channel name".to_string(),
                config_schema: r#"{"type":"object","properties":{"channels":{"type":"array","items":{"type":"string"}}}}"#.to_string(),
            },
            FilterType {
                id: "user_role_filter".to_string(),
                name: "User Role Filter".to_string(),
                description: "Filter events by user roles".to_string(),
                config_schema: r#"{"type":"object","properties":{"required_roles":{"type":"array","items":{"type":"string"}},"match_any":{"type":"boolean"}}}"#.to_string(),
            },
            FilterType {
                id: "user_level_filter".to_string(),
                name: "User Level Filter".to_string(),
                description: "Filter events by user level (viewer, subscriber, moderator, etc.)".to_string(),
                config_schema: r#"{"type":"object","properties":{"min_level":{"type":"string"},"allowed_levels":{"type":"array","items":{"type":"string"}}}}"#.to_string(),
            },
            FilterType {
                id: "message_pattern_filter".to_string(),
                name: "Message Pattern Filter".to_string(),
                description: "Filter events by message content using regex patterns".to_string(),
                config_schema: r#"{"type":"object","properties":{"patterns":{"type":"array","items":{"type":"string"}},"match_any":{"type":"boolean"},"case_insensitive":{"type":"boolean"}}}"#.to_string(),
            },
            FilterType {
                id: "message_length_filter".to_string(),
                name: "Message Length Filter".to_string(),
                description: "Filter events by message length".to_string(),
                config_schema: r#"{"type":"object","properties":{"min_length":{"type":"integer"},"max_length":{"type":"integer"}}}"#.to_string(),
            },
            FilterType {
                id: "time_window_filter".to_string(),
                name: "Time Window Filter".to_string(),
                description: "Filter events by time of day and day of week".to_string(),
                config_schema: r#"{"type":"object","properties":{"start_hour":{"type":"integer"},"end_hour":{"type":"integer"},"timezone":{"type":"string"},"days_of_week":{"type":"array","items":{"type":"integer"}}}}"#.to_string(),
            },
            FilterType {
                id: "cooldown_filter".to_string(),
                name: "Cooldown Filter".to_string(),
                description: "Prevent rapid repeated executions".to_string(),
                config_schema: r#"{"type":"object","properties":{"cooldown_seconds":{"type":"integer"},"per_user":{"type":"boolean"},"per_channel":{"type":"boolean"}}}"#.to_string(),
            },
        ];
        
        Ok(Response::new(GetAvailableFiltersResponse {
            success: true,
            message: format!("Found {} available filter types", filters.len()),
            filters,
        }))
    }
    
    async fn get_available_actions(
        &self,
        _request: Request<GetAvailableActionsRequest>,
    ) -> Result<Response<GetAvailableActionsResponse>, Status> {
        debug!("Getting available action types");
        
        // TODO: Get these from the EventPipelineService registry
        let actions = vec![
            ActionType {
                id: "log_action".to_string(),
                name: "Log Event".to_string(),
                description: "Log event details at a specified level".to_string(),
                config_schema: r#"{"type":"object","properties":{"level":{"type":"string","enum":["error","warn","info","debug","trace"]},"prefix":{"type":"string"}}}"#.to_string(),
                is_parallelizable: true,
            },
            ActionType {
                id: "discord_message".to_string(),
                name: "Send Discord Message".to_string(),
                description: "Send a message to a Discord channel".to_string(),
                config_schema: r#"{"type":"object","properties":{"account":{"type":"string"},"guild_id":{"type":"string"},"channel_id":{"type":"string"},"message_template":{"type":"string"}}}"#.to_string(),
                is_parallelizable: false,
            },
            ActionType {
                id: "discord_role_add".to_string(),
                name: "Add Discord Role".to_string(),
                description: "Add a role to a Discord user".to_string(),
                config_schema: r#"{"type":"object","properties":{"account":{"type":"string"},"guild_id":{"type":"string"},"role_id":{"type":"string"},"reason":{"type":"string"}}}"#.to_string(),
                is_parallelizable: false,
            },
            ActionType {
                id: "discord_role_remove".to_string(),
                name: "Remove Discord Role".to_string(),
                description: "Remove a role from a Discord user".to_string(),
                config_schema: r#"{"type":"object","properties":{"account":{"type":"string"},"guild_id":{"type":"string"},"role_id":{"type":"string"},"reason":{"type":"string"}}}"#.to_string(),
                is_parallelizable: false,
            },
            ActionType {
                id: "twitch_message".to_string(),
                name: "Send Twitch Message".to_string(),
                description: "Send a message to a Twitch chat".to_string(),
                config_schema: r#"{"type":"object","properties":{"account":{"type":"string"},"channel":{"type":"string"},"message_template":{"type":"string"},"reply_to_message":{"type":"boolean"}}}"#.to_string(),
                is_parallelizable: false,
            },
            ActionType {
                id: "twitch_timeout".to_string(),
                name: "Timeout Twitch User".to_string(),
                description: "Timeout a user in Twitch chat".to_string(),
                config_schema: r#"{"type":"object","properties":{"account":{"type":"string"},"channel":{"type":"string"},"duration_seconds":{"type":"integer"},"reason":{"type":"string"}}}"#.to_string(),
                is_parallelizable: false,
            },
            ActionType {
                id: "osc_trigger".to_string(),
                name: "Trigger OSC Parameter".to_string(),
                description: "Trigger an OSC parameter".to_string(),
                config_schema: r#"{"type":"object","properties":{"parameter_path":{"type":"string"},"value":{"type":"number"},"duration_ms":{"type":"integer"},"toggle_id":{"type":"string"}}}"#.to_string(),
                is_parallelizable: true,
            },
            ActionType {
                id: "obs_scene_change".to_string(),
                name: "Change OBS Scene".to_string(),
                description: "Change the active scene in OBS".to_string(),
                config_schema: r#"{"type":"object","properties":{"instance_name":{"type":"string"},"scene_name":{"type":"string"},"transition_name":{"type":"string"},"transition_duration_ms":{"type":"integer"}}}"#.to_string(),
                is_parallelizable: false,
            },
            ActionType {
                id: "obs_source_toggle".to_string(),
                name: "Toggle OBS Source".to_string(),
                description: "Toggle visibility of an OBS source".to_string(),
                config_schema: r#"{"type":"object","properties":{"instance_name":{"type":"string"},"scene_name":{"type":"string"},"source_name":{"type":"string"},"action":{"type":"string","enum":["toggle","show","hide"]}}}"#.to_string(),
                is_parallelizable: false,
            },
            ActionType {
                id: "plugin_call".to_string(),
                name: "Call Plugin Function".to_string(),
                description: "Execute a plugin function".to_string(),
                config_schema: r#"{"type":"object","properties":{"plugin_id":{"type":"string"},"function_name":{"type":"string"},"parameters":{"type":"object"},"pass_event":{"type":"boolean"}}}"#.to_string(),
                is_parallelizable: false,
            },
            ActionType {
                id: "ai_respond".to_string(),
                name: "AI Respond".to_string(),
                description: "Generate an AI response".to_string(),
                config_schema: r#"{"type":"object","properties":{"provider_id":{"type":"string"},"model":{"type":"string"},"system_prompt":{"type":"string"},"prompt_template":{"type":"string"},"max_tokens":{"type":"integer"},"temperature":{"type":"number"},"send_response":{"type":"boolean"},"response_prefix":{"type":"string"}}}"#.to_string(),
                is_parallelizable: false,
            },
        ];
        
        Ok(Response::new(GetAvailableActionsResponse {
            success: true,
            message: format!("Found {} available action types", actions.len()),
            actions,
        }))
    }
    
    async fn get_execution_history(
        &self,
        request: Request<GetExecutionHistoryRequest>,
    ) -> Result<Response<GetExecutionHistoryResponse>, Status> {
        let req = request.into_inner();
        debug!("Getting execution history");
        
        let limit = req.limit.unwrap_or(100) as i64;
        let offset = req.offset.unwrap_or(0) as i64;
        
        // Parse optional pipeline ID
        let pipeline_id = if let Some(id_str) = req.pipeline_id {
            Some(match Uuid::parse_str(&id_str) {
                Ok(id) => id,
                Err(e) => {
                    return Ok(Response::new(GetExecutionHistoryResponse {
                        success: false,
                        message: format!("Invalid pipeline ID: {}", e),
                        executions: vec![],
                        total_count: 0,
                    }));
                }
            })
        } else {
            None
        };
        
        // Use the appropriate repository method
        let executions_result = if let Some(pipeline_id) = pipeline_id {
            self.ctx.event_pipeline_service.repository.list_executions_for_pipeline(pipeline_id, limit, None).await
        } else {
            self.ctx.event_pipeline_service.repository.list_recent_executions(limit).await
        };
        
        match executions_result {
            Ok(executions) => {
                let mut proto_executions = Vec::new();
                
                for exec in executions {
                    let action_results: Vec<ActionResult> = exec.action_results
                        .iter()
                        .map(|result| ActionResult {
                            action_id: result.action_id.to_string(),
                            action_type: result.action_type.clone(),
                            status: format!("{:?}", result.status),
                            output: result.output_data.as_ref().map(|v| v.to_string()).unwrap_or_default(),
                            error: result.error_message.clone().unwrap_or_default(),
                            started_at: result.started_at.to_rfc3339(),
                            completed_at: result.completed_at.map(|dt| dt.to_rfc3339()).unwrap_or_default(),
                        })
                        .collect();
                    
                    proto_executions.push(ExecutionLog {
                        execution_id: exec.execution_id.to_string(),
                        pipeline_id: exec.pipeline_id.to_string(),
                        pipeline_name: "".to_string(), // TODO: Get from join
                        event_type: exec.event_type,
                        event_data: exec.event_data.to_string(),
                        status: format!("{:?}", exec.status),
                        error_message: exec.error_message.unwrap_or_default(),
                        started_at: exec.started_at.to_rfc3339(),
                        completed_at: exec.completed_at.map(|dt| dt.to_rfc3339()).unwrap_or_default(),
                        action_results,
                    });
                }
                
                Ok(Response::new(GetExecutionHistoryResponse {
                    success: true,
                    message: format!("Found {} executions", proto_executions.len()),
                    executions: proto_executions,
                    total_count: 0, // TODO: Get total count
                }))
            }
            Err(e) => {
                error!("Failed to get execution history: {:?}", e);
                Ok(Response::new(GetExecutionHistoryResponse {
                    success: false,
                    message: format!("Failed to get execution history: {}", e),
                    executions: vec![],
                    total_count: 0,
                }))
            }
        }
    }
    
    async fn get_execution_details(
        &self,
        request: Request<GetExecutionDetailsRequest>,
    ) -> Result<Response<GetExecutionDetailsResponse>, Status> {
        let req = request.into_inner();
        debug!("Getting execution details for: {}", req.execution_id);
        
        let execution_id = match Uuid::parse_str(&req.execution_id) {
            Ok(id) => id,
            Err(e) => {
                return Ok(Response::new(GetExecutionDetailsResponse {
                    success: false,
                    message: format!("Invalid execution ID: {}", e),
                    execution: None,
                }));
            }
        };
        
        match self.ctx.event_pipeline_service.repository.get_execution(execution_id).await {
            Ok(Some(exec)) => {
                let action_results: Vec<ActionResult> = exec.action_results
                    .iter()
                    .map(|result| ActionResult {
                        action_id: result.action_id.to_string(),
                        action_type: result.action_type.clone(),
                        status: format!("{:?}", result.status),
                        output: result.output_data.as_ref().map(|v| v.to_string()).unwrap_or_default(),
                        error: result.error_message.clone().unwrap_or_default(),
                        started_at: result.started_at.to_rfc3339(),
                        completed_at: result.completed_at.map(|dt| dt.to_rfc3339()).unwrap_or_default(),
                    })
                    .collect();
                
                let proto_exec = ExecutionLog {
                    execution_id: exec.execution_id.to_string(),
                    pipeline_id: exec.pipeline_id.to_string(),
                    pipeline_name: "".to_string(), // TODO: Get from join
                    event_type: exec.event_type,
                    event_data: exec.event_data.to_string(),
                    status: format!("{:?}", exec.status),
                    error_message: exec.error_message.unwrap_or_default(),
                    started_at: exec.started_at.to_rfc3339(),
                    completed_at: exec.completed_at.map(|dt| dt.to_rfc3339()).unwrap_or_default(),
                    action_results,
                };
                
                Ok(Response::new(GetExecutionDetailsResponse {
                    success: true,
                    message: "Execution details retrieved successfully".to_string(),
                    execution: Some(proto_exec),
                }))
            }
            Ok(None) => {
                Ok(Response::new(GetExecutionDetailsResponse {
                    success: false,
                    message: format!("Execution with ID {} not found", req.execution_id),
                    execution: None,
                }))
            }
            Err(e) => {
                error!("Failed to get execution details: {:?}", e);
                Ok(Response::new(GetExecutionDetailsResponse {
                    success: false,
                    message: format!("Failed to get execution details: {}", e),
                    execution: None,
                }))
            }
        }
    }
    
    async fn reload_pipelines(
        &self,
        _request: Request<ReloadPipelinesRequest>,
    ) -> Result<Response<ReloadPipelinesResponse>, Status> {
        info!("Reloading pipelines");
        
        match self.ctx.event_pipeline_service.reload_pipelines().await {
            Ok(_) => {
                let count = self.ctx.event_pipeline_service.pipeline_count().await as i32;
                
                Ok(Response::new(ReloadPipelinesResponse {
                    success: true,
                    message: format!("Successfully reloaded {} pipelines", count),
                    pipelines_loaded: count,
                }))
            }
            Err(e) => {
                error!("Failed to reload pipelines: {:?}", e);
                Ok(Response::new(ReloadPipelinesResponse {
                    success: false,
                    message: format!("Failed to reload pipelines: {}", e),
                    pipelines_loaded: 0,
                }))
            }
        }
    }
}