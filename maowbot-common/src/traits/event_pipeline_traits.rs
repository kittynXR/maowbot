use async_trait::async_trait;
use chrono::{DateTime, Utc};
use uuid::Uuid;
use crate::error::Error;
use crate::models::event_pipeline::{
    EventPipeline, PipelineFilter, PipelineAction, PipelineExecutionLog,
    PipelineExecutionStatus, PipelineSharedData, EventTypeRegistry, EventHandlerRegistry,
    CreatePipelineRequest, UpdatePipelineRequest, CreateFilterRequest, CreateActionRequest,
    HandlerType,
};

/// Repository trait for managing event pipelines
#[async_trait]
pub trait EventPipelineRepository: Send + Sync {
    // Pipeline CRUD operations
    async fn create_pipeline(&self, request: &CreatePipelineRequest) -> Result<EventPipeline, Error>;
    async fn get_pipeline(&self, pipeline_id: Uuid) -> Result<Option<EventPipeline>, Error>;
    async fn get_pipeline_by_name(&self, name: &str) -> Result<Option<EventPipeline>, Error>;
    async fn list_pipelines(&self, enabled_only: bool) -> Result<Vec<EventPipeline>, Error>;
    async fn list_pipelines_by_tag(&self, tag: &str) -> Result<Vec<EventPipeline>, Error>;
    async fn update_pipeline(&self, pipeline_id: Uuid, request: &UpdatePipelineRequest) -> Result<EventPipeline, Error>;
    async fn delete_pipeline(&self, pipeline_id: Uuid) -> Result<(), Error>;
    
    // Filter operations
    async fn add_filter(&self, pipeline_id: Uuid, request: &CreateFilterRequest) -> Result<PipelineFilter, Error>;
    async fn get_filter(&self, filter_id: Uuid) -> Result<Option<PipelineFilter>, Error>;
    async fn list_filters_for_pipeline(&self, pipeline_id: Uuid) -> Result<Vec<PipelineFilter>, Error>;
    async fn update_filter(&self, filter_id: Uuid, request: &CreateFilterRequest) -> Result<PipelineFilter, Error>;
    async fn delete_filter(&self, filter_id: Uuid) -> Result<(), Error>;
    async fn reorder_filters(&self, pipeline_id: Uuid, filter_ids: Vec<Uuid>) -> Result<(), Error>;
    
    // Action operations
    async fn add_action(&self, pipeline_id: Uuid, request: &CreateActionRequest) -> Result<PipelineAction, Error>;
    async fn get_action(&self, action_id: Uuid) -> Result<Option<PipelineAction>, Error>;
    async fn list_actions_for_pipeline(&self, pipeline_id: Uuid) -> Result<Vec<PipelineAction>, Error>;
    async fn update_action(&self, action_id: Uuid, request: &CreateActionRequest) -> Result<PipelineAction, Error>;
    async fn delete_action(&self, action_id: Uuid) -> Result<(), Error>;
    async fn reorder_actions(&self, pipeline_id: Uuid, action_ids: Vec<Uuid>) -> Result<(), Error>;
    
    // Execution operations
    async fn get_pipelines_for_event(&self, event_type: &str, platform: &str) -> Result<Vec<EventPipeline>, Error>;
    async fn increment_execution_stats(&self, pipeline_id: Uuid, success: bool) -> Result<(), Error>;
}

/// Repository trait for managing pipeline execution logs
#[async_trait]
pub trait PipelineExecutionLogRepository: Send + Sync {
    async fn create_execution(&self, pipeline_id: Uuid, event_type: &str, event_data: serde_json::Value) -> Result<PipelineExecutionLog, Error>;
    async fn get_execution(&self, execution_id: Uuid) -> Result<Option<PipelineExecutionLog>, Error>;
    async fn update_execution_status(
        &self, 
        execution_id: Uuid, 
        status: PipelineExecutionStatus, 
        error_message: Option<String>
    ) -> Result<(), Error>;
    async fn add_action_result(
        &self,
        execution_id: Uuid,
        action_result: serde_json::Value
    ) -> Result<(), Error>;
    async fn list_executions_for_pipeline(
        &self, 
        pipeline_id: Uuid, 
        limit: i64,
        status_filter: Option<PipelineExecutionStatus>
    ) -> Result<Vec<PipelineExecutionLog>, Error>;
    async fn list_recent_executions(&self, limit: i64) -> Result<Vec<PipelineExecutionLog>, Error>;
    async fn cleanup_old_executions(&self, older_than: DateTime<Utc>) -> Result<i64, Error>;
}

/// Repository trait for managing pipeline shared data
#[async_trait]
pub trait PipelineSharedDataRepository: Send + Sync {
    async fn set_shared_data(
        &self,
        execution_id: Uuid,
        key: &str,
        value: serde_json::Value,
        data_type: Option<String>,
        set_by_action: Option<Uuid>
    ) -> Result<(), Error>;
    async fn get_shared_data(&self, execution_id: Uuid, key: &str) -> Result<Option<PipelineSharedData>, Error>;
    async fn list_shared_data(&self, execution_id: Uuid) -> Result<Vec<PipelineSharedData>, Error>;
    async fn delete_shared_data(&self, execution_id: Uuid, key: &str) -> Result<(), Error>;
}

/// Repository trait for managing event type registry
#[async_trait]
pub trait EventTypeRegistryRepository: Send + Sync {
    async fn register_event_type(
        &self,
        platform: &str,
        category: &str,
        name: &str,
        description: Option<String>,
        schema: Option<serde_json::Value>
    ) -> Result<EventTypeRegistry, Error>;
    async fn get_event_type(&self, event_type_id: Uuid) -> Result<Option<EventTypeRegistry>, Error>;
    async fn get_event_type_by_name(&self, platform: &str, event_name: &str) -> Result<Option<EventTypeRegistry>, Error>;
    async fn list_event_types(&self, platform: Option<&str>) -> Result<Vec<EventTypeRegistry>, Error>;
    async fn update_event_type(&self, event_type_id: Uuid, enabled: bool) -> Result<(), Error>;
    async fn delete_event_type(&self, event_type_id: Uuid) -> Result<(), Error>;
}

/// Repository trait for managing event handler registry
#[async_trait]
pub trait EventHandlerRegistryRepository: Send + Sync {
    async fn register_handler(
        &self,
        handler_type: HandlerType,
        name: &str,
        category: &str,
        description: Option<String>,
        parameters: Option<serde_json::Value>,
        plugin_id: Option<String>
    ) -> Result<EventHandlerRegistry, Error>;
    async fn get_handler(&self, handler_id: Uuid) -> Result<Option<EventHandlerRegistry>, Error>;
    async fn get_handler_by_name(&self, handler_name: &str) -> Result<Option<EventHandlerRegistry>, Error>;
    async fn list_handlers(&self, handler_type: Option<HandlerType>) -> Result<Vec<EventHandlerRegistry>, Error>;
    async fn list_handlers_by_category(&self, category: &str) -> Result<Vec<EventHandlerRegistry>, Error>;
    async fn update_handler(&self, handler_id: Uuid, enabled: bool) -> Result<(), Error>;
    async fn delete_handler(&self, handler_id: Uuid) -> Result<(), Error>;
}

/// Combined repository trait for all event pipeline operations
#[async_trait]
pub trait EventPipelineSystemRepository: 
    EventPipelineRepository + 
    PipelineExecutionLogRepository + 
    PipelineSharedDataRepository +
    EventTypeRegistryRepository +
    EventHandlerRegistryRepository
{
    // Additional convenience methods that span multiple repositories
    
    /// Get a pipeline with all its filters and actions
    async fn get_pipeline_with_details(&self, pipeline_id: Uuid) -> Result<Option<(EventPipeline, Vec<PipelineFilter>, Vec<PipelineAction>)>, Error> {
        if let Some(pipeline) = self.get_pipeline(pipeline_id).await? {
            let filters = self.list_filters_for_pipeline(pipeline_id).await?;
            let actions = self.list_actions_for_pipeline(pipeline_id).await?;
            Ok(Some((pipeline, filters, actions)))
        } else {
            Ok(None)
        }
    }
    
    /// Clone a pipeline with all its filters and actions
    async fn clone_pipeline(&self, pipeline_id: Uuid, new_name: &str) -> Result<EventPipeline, Error> {
        if let Some((original, filters, actions)) = self.get_pipeline_with_details(pipeline_id).await? {
            // Create new pipeline
            let mut request = CreatePipelineRequest {
                name: new_name.to_string(),
                description: original.description.map(|d| format!("Clone of: {}", d)),
                enabled: false, // Start disabled
                priority: original.priority,
                stop_on_match: original.stop_on_match,
                stop_on_error: original.stop_on_error,
                tags: original.tags.clone(),
                metadata: Some(original.metadata.clone()),
            };
            
            let new_pipeline = self.create_pipeline(&request).await?;
            
            // Clone filters
            for filter in filters {
                let filter_request = CreateFilterRequest {
                    filter_type: filter.filter_type,
                    filter_config: filter.filter_config,
                    filter_order: filter.filter_order,
                    is_negated: filter.is_negated,
                    is_required: filter.is_required,
                };
                self.add_filter(new_pipeline.pipeline_id, &filter_request).await?;
            }
            
            // Clone actions
            for action in actions {
                let action_request = CreateActionRequest {
                    action_type: action.action_type,
                    action_config: action.action_config,
                    action_order: action.action_order,
                    continue_on_error: action.continue_on_error,
                    is_async: action.is_async,
                    timeout_ms: action.timeout_ms,
                    retry_count: action.retry_count,
                    retry_delay_ms: action.retry_delay_ms,
                    condition_type: action.condition_type,
                    condition_config: action.condition_config,
                };
                self.add_action(new_pipeline.pipeline_id, &action_request).await?;
            }
            
            Ok(new_pipeline)
        } else {
            Err(Error::NotFound("Pipeline not found".to_string()))
        }
    }
}