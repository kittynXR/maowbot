use crate::{GrpcClient, CommandResult, CommandError};
use maowbot_proto::maowbot::services::event_pipeline::{
    CreatePipelineRequest, UpdatePipelineRequest, DeletePipelineRequest,
    GetPipelineRequest, ListPipelinesRequest, TogglePipelineRequest,
    AddFilterRequest, UpdateFilterRequest, RemoveFilterRequest, ListFiltersRequest,
    AddActionRequest, UpdateActionRequest, RemoveActionRequest, ListActionsRequest,
    GetAvailableFiltersRequest, GetAvailableActionsRequest,
    GetExecutionHistoryRequest, GetExecutionDetailsRequest,
    ReloadPipelinesRequest,
    Pipeline, PipelineFilter, PipelineAction, FilterType, ActionType, ExecutionLog,
};

// Result structures
pub struct CreatePipelineResult {
    pub pipeline: Pipeline,
}

pub struct UpdatePipelineResult {
    pub pipeline: Pipeline,
}

pub struct DeletePipelineResult {
    pub success: bool,
}

pub struct GetPipelineResult {
    pub pipeline: Pipeline,
}

pub struct ListPipelinesResult {
    pub pipelines: Vec<Pipeline>,
}

pub struct TogglePipelineResult {
    pub success: bool,
}

pub struct AddFilterResult {
    pub filter: PipelineFilter,
}

pub struct UpdateFilterResult {
    pub filter: PipelineFilter,
}

pub struct RemoveFilterResult {
    pub success: bool,
}

pub struct ListFiltersResult {
    pub filters: Vec<PipelineFilter>,
}

pub struct AddActionResult {
    pub action: PipelineAction,
}

pub struct UpdateActionResult {
    pub action: PipelineAction,
}

pub struct RemoveActionResult {
    pub success: bool,
}

pub struct ListActionsResult {
    pub actions: Vec<PipelineAction>,
}

pub struct GetAvailableFiltersResult {
    pub filters: Vec<FilterType>,
}

pub struct GetAvailableActionsResult {
    pub actions: Vec<ActionType>,
}

pub struct GetExecutionHistoryResult {
    pub executions: Vec<ExecutionLog>,
    pub total_count: i32,
}

pub struct GetExecutionDetailsResult {
    pub execution: ExecutionLog,
}

pub struct ReloadPipelinesResult {
    pub pipelines_loaded: i32,
}

// Command handlers
pub struct PipelineCommands;

impl PipelineCommands {
    pub async fn create_pipeline(
        client: &GrpcClient,
        name: &str,
        description: &str,
        priority: i32,
        stop_on_match: bool,
        stop_on_error: bool,
        tags: Vec<String>,
    ) -> Result<CommandResult<CreatePipelineResult>, CommandError> {
        let request = CreatePipelineRequest {
            name: name.to_string(),
            description: description.to_string(),
            priority,
            stop_on_match,
            stop_on_error,
            tags,
        };

        let response = client.pipeline.clone()
            .create_pipeline(request)
            .await
            .map_err(|e| CommandError::GrpcError(e.to_string()))?;

        let inner = response.into_inner();
        if !inner.success {
            return Err(CommandError::DataError(inner.message));
        }

        let pipeline = inner.pipeline
            .ok_or_else(|| CommandError::DataError("No pipeline returned".to_string()))?;

        Ok(CommandResult::new(CreatePipelineResult { pipeline }))
    }

    pub async fn update_pipeline(
        client: &GrpcClient,
        pipeline_id: &str,
        name: Option<&str>,
        description: Option<&str>,
        priority: Option<i32>,
        stop_on_match: Option<bool>,
        stop_on_error: Option<bool>,
        enabled: Option<bool>,
    ) -> Result<CommandResult<UpdatePipelineResult>, CommandError> {
        let request = UpdatePipelineRequest {
            pipeline_id: pipeline_id.to_string(),
            name: name.map(|s| s.to_string()),
            description: description.map(|s| s.to_string()),
            priority,
            stop_on_match,
            stop_on_error,
            enabled,
        };

        let response = client.pipeline.clone()
            .update_pipeline(request)
            .await
            .map_err(|e| CommandError::GrpcError(e.to_string()))?;

        let inner = response.into_inner();
        if !inner.success {
            return Err(CommandError::DataError(inner.message));
        }

        let pipeline = inner.pipeline
            .ok_or_else(|| CommandError::DataError("No pipeline returned".to_string()))?;

        Ok(CommandResult::new(UpdatePipelineResult { pipeline }))
    }

    pub async fn delete_pipeline(
        client: &GrpcClient,
        pipeline_id: &str,
    ) -> Result<CommandResult<DeletePipelineResult>, CommandError> {
        let request = DeletePipelineRequest { pipeline_id: pipeline_id.to_string() };

        let response = client.pipeline.clone()
            .delete_pipeline(request)
            .await
            .map_err(|e| CommandError::GrpcError(e.to_string()))?;

        let inner = response.into_inner();
        if !inner.success {
            return Err(CommandError::DataError(inner.message));
        }

        Ok(CommandResult::new(DeletePipelineResult { success: true }))
    }

    pub async fn get_pipeline(
        client: &GrpcClient,
        pipeline_id: &str,
    ) -> Result<CommandResult<GetPipelineResult>, CommandError> {
        let request = GetPipelineRequest { pipeline_id: pipeline_id.to_string() };

        let response = client.pipeline.clone()
            .get_pipeline(request)
            .await
            .map_err(|e| CommandError::GrpcError(e.to_string()))?;

        let inner = response.into_inner();
        if !inner.success {
            return Err(CommandError::DataError(inner.message));
        }

        let pipeline = inner.pipeline
            .ok_or_else(|| CommandError::DataError("No pipeline returned".to_string()))?;

        Ok(CommandResult::new(GetPipelineResult { pipeline }))
    }

    pub async fn list_pipelines(
        client: &GrpcClient,
        include_disabled: bool,
    ) -> Result<CommandResult<ListPipelinesResult>, CommandError> {
        let request = ListPipelinesRequest { include_disabled };

        let response = client.pipeline.clone()
            .list_pipelines(request)
            .await
            .map_err(|e| CommandError::GrpcError(e.to_string()))?;

        let inner = response.into_inner();
        if !inner.success {
            return Err(CommandError::DataError(inner.message));
        }

        Ok(CommandResult::new(ListPipelinesResult { 
            pipelines: inner.pipelines 
        }))
    }

    pub async fn toggle_pipeline(
        client: &GrpcClient,
        pipeline_id: &str,
        enabled: bool,
    ) -> Result<CommandResult<TogglePipelineResult>, CommandError> {
        let request = TogglePipelineRequest { pipeline_id: pipeline_id.to_string(), enabled };

        let response = client.pipeline.clone()
            .toggle_pipeline(request)
            .await
            .map_err(|e| CommandError::GrpcError(e.to_string()))?;

        let inner = response.into_inner();
        if !inner.success {
            return Err(CommandError::DataError(inner.message));
        }

        Ok(CommandResult::new(TogglePipelineResult { success: true }))
    }

    pub async fn add_filter(
        client: &GrpcClient,
        pipeline_id: &str,
        filter_type: &str,
        filter_config: &str,
        filter_order: Option<i32>,
        is_negated: bool,
        is_required: bool,
    ) -> Result<CommandResult<AddFilterResult>, CommandError> {
        let request = AddFilterRequest {
            pipeline_id: pipeline_id.to_string(),
            filter_type: filter_type.to_string(),
            filter_config: filter_config.to_string(),
            filter_order,
            is_negated,
            is_required,
        };

        let response = client.pipeline.clone()
            .add_filter(request)
            .await
            .map_err(|e| CommandError::GrpcError(e.to_string()))?;

        let inner = response.into_inner();
        if !inner.success {
            return Err(CommandError::DataError(inner.message));
        }

        let filter = inner.filter
            .ok_or_else(|| CommandError::DataError("No filter returned".to_string()))?;

        Ok(CommandResult::new(AddFilterResult { filter }))
    }

    pub async fn update_filter(
        client: &GrpcClient,
        filter_id: &str,
        filter_config: Option<&str>,
        filter_order: Option<i32>,
        is_negated: Option<bool>,
        is_required: Option<bool>,
    ) -> Result<CommandResult<UpdateFilterResult>, CommandError> {
        let request = UpdateFilterRequest {
            filter_id: filter_id.to_string(),
            filter_config: filter_config.map(|s| s.to_string()),
            filter_order,
            is_negated,
            is_required,
        };

        let response = client.pipeline.clone()
            .update_filter(request)
            .await
            .map_err(|e| CommandError::GrpcError(e.to_string()))?;

        let inner = response.into_inner();
        if !inner.success {
            return Err(CommandError::DataError(inner.message));
        }

        let filter = inner.filter
            .ok_or_else(|| CommandError::DataError("No filter returned".to_string()))?;

        Ok(CommandResult::new(UpdateFilterResult { filter }))
    }

    pub async fn remove_filter(
        client: &GrpcClient,
        filter_id: &str,
    ) -> Result<CommandResult<RemoveFilterResult>, CommandError> {
        let request = RemoveFilterRequest { filter_id: filter_id.to_string() };

        let response = client.pipeline.clone()
            .remove_filter(request)
            .await
            .map_err(|e| CommandError::GrpcError(e.to_string()))?;

        let inner = response.into_inner();
        if !inner.success {
            return Err(CommandError::DataError(inner.message));
        }

        Ok(CommandResult::new(RemoveFilterResult { success: true }))
    }

    pub async fn list_filters(
        client: &GrpcClient,
        pipeline_id: &str,
    ) -> Result<CommandResult<ListFiltersResult>, CommandError> {
        let request = ListFiltersRequest { pipeline_id: pipeline_id.to_string() };

        let response = client.pipeline.clone()
            .list_filters(request)
            .await
            .map_err(|e| CommandError::GrpcError(e.to_string()))?;

        let inner = response.into_inner();
        if !inner.success {
            return Err(CommandError::DataError(inner.message));
        }

        Ok(CommandResult::new(ListFiltersResult { 
            filters: inner.filters 
        }))
    }

    pub async fn add_action(
        client: &GrpcClient,
        pipeline_id: &str,
        action_type: &str,
        action_config: &str,
        action_order: Option<i32>,
        continue_on_error: bool,
        is_async: bool,
        timeout_ms: Option<i32>,
        retry_count: i32,
        retry_delay_ms: i32,
    ) -> Result<CommandResult<AddActionResult>, CommandError> {
        let request = AddActionRequest {
            pipeline_id: pipeline_id.to_string(),
            action_type: action_type.to_string(),
            action_config: action_config.to_string(),
            action_order,
            continue_on_error,
            is_async,
            timeout_ms,
            retry_count,
            retry_delay_ms,
        };

        let response = client.pipeline.clone()
            .add_action(request)
            .await
            .map_err(|e| CommandError::GrpcError(e.to_string()))?;

        let inner = response.into_inner();
        if !inner.success {
            return Err(CommandError::DataError(inner.message));
        }

        let action = inner.action
            .ok_or_else(|| CommandError::DataError("No action returned".to_string()))?;

        Ok(CommandResult::new(AddActionResult { action }))
    }

    pub async fn update_action(
        client: &GrpcClient,
        action_id: &str,
        action_config: Option<&str>,
        action_order: Option<i32>,
        continue_on_error: Option<bool>,
        is_async: Option<bool>,
        timeout_ms: Option<i32>,
        retry_count: Option<i32>,
        retry_delay_ms: Option<i32>,
    ) -> Result<CommandResult<UpdateActionResult>, CommandError> {
        let request = UpdateActionRequest {
            action_id: action_id.to_string(),
            action_config: action_config.map(|s| s.to_string()),
            action_order,
            continue_on_error,
            is_async,
            timeout_ms,
            retry_count,
            retry_delay_ms,
        };

        let response = client.pipeline.clone()
            .update_action(request)
            .await
            .map_err(|e| CommandError::GrpcError(e.to_string()))?;

        let inner = response.into_inner();
        if !inner.success {
            return Err(CommandError::DataError(inner.message));
        }

        let action = inner.action
            .ok_or_else(|| CommandError::DataError("No action returned".to_string()))?;

        Ok(CommandResult::new(UpdateActionResult { action }))
    }

    pub async fn remove_action(
        client: &GrpcClient,
        action_id: &str,
    ) -> Result<CommandResult<RemoveActionResult>, CommandError> {
        let request = RemoveActionRequest { action_id: action_id.to_string() };

        let response = client.pipeline.clone()
            .remove_action(request)
            .await
            .map_err(|e| CommandError::GrpcError(e.to_string()))?;

        let inner = response.into_inner();
        if !inner.success {
            return Err(CommandError::DataError(inner.message));
        }

        Ok(CommandResult::new(RemoveActionResult { success: true }))
    }

    pub async fn list_actions(
        client: &GrpcClient,
        pipeline_id: &str,
    ) -> Result<CommandResult<ListActionsResult>, CommandError> {
        let request = ListActionsRequest { pipeline_id: pipeline_id.to_string() };

        let response = client.pipeline.clone()
            .list_actions(request)
            .await
            .map_err(|e| CommandError::GrpcError(e.to_string()))?;

        let inner = response.into_inner();
        if !inner.success {
            return Err(CommandError::DataError(inner.message));
        }

        Ok(CommandResult::new(ListActionsResult { 
            actions: inner.actions 
        }))
    }

    pub async fn get_available_filters(
        client: &GrpcClient,
    ) -> Result<CommandResult<GetAvailableFiltersResult>, CommandError> {
        let request = GetAvailableFiltersRequest {};

        let response = client.pipeline.clone()
            .get_available_filters(request)
            .await
            .map_err(|e| CommandError::GrpcError(e.to_string()))?;

        let inner = response.into_inner();
        if !inner.success {
            return Err(CommandError::DataError(inner.message));
        }

        Ok(CommandResult::new(GetAvailableFiltersResult { 
            filters: inner.filters 
        }))
    }

    pub async fn get_available_actions(
        client: &GrpcClient,
    ) -> Result<CommandResult<GetAvailableActionsResult>, CommandError> {
        let request = GetAvailableActionsRequest {};

        let response = client.pipeline.clone()
            .get_available_actions(request)
            .await
            .map_err(|e| CommandError::GrpcError(e.to_string()))?;

        let inner = response.into_inner();
        if !inner.success {
            return Err(CommandError::DataError(inner.message));
        }

        Ok(CommandResult::new(GetAvailableActionsResult { 
            actions: inner.actions 
        }))
    }

    pub async fn get_execution_history(
        client: &GrpcClient,
        pipeline_id: Option<&str>,
        limit: Option<i32>,
        offset: Option<i32>,
    ) -> Result<CommandResult<GetExecutionHistoryResult>, CommandError> {
        let request = GetExecutionHistoryRequest {
            pipeline_id: pipeline_id.map(|s| s.to_string()),
            limit,
            offset,
        };

        let response = client.pipeline.clone()
            .get_execution_history(request)
            .await
            .map_err(|e| CommandError::GrpcError(e.to_string()))?;

        let inner = response.into_inner();
        if !inner.success {
            return Err(CommandError::DataError(inner.message));
        }

        Ok(CommandResult::new(GetExecutionHistoryResult { 
            executions: inner.executions,
            total_count: inner.total_count,
        }))
    }

    pub async fn get_execution_details(
        client: &GrpcClient,
        execution_id: &str,
    ) -> Result<CommandResult<GetExecutionDetailsResult>, CommandError> {
        let request = GetExecutionDetailsRequest {
            execution_id: execution_id.to_string(),
        };

        let response = client.pipeline.clone()
            .get_execution_details(request)
            .await
            .map_err(|e| CommandError::GrpcError(e.to_string()))?;

        let inner = response.into_inner();
        if !inner.success {
            return Err(CommandError::DataError(inner.message));
        }

        let execution = inner.execution
            .ok_or_else(|| CommandError::DataError("No execution returned".to_string()))?;

        Ok(CommandResult::new(GetExecutionDetailsResult { execution }))
    }

    pub async fn reload_pipelines(
        client: &GrpcClient,
    ) -> Result<CommandResult<ReloadPipelinesResult>, CommandError> {
        let request = ReloadPipelinesRequest {};

        let response = client.pipeline.clone()
            .reload_pipelines(request)
            .await
            .map_err(|e| CommandError::GrpcError(e.to_string()))?;

        let inner = response.into_inner();
        if !inner.success {
            return Err(CommandError::DataError(inner.message));
        }

        Ok(CommandResult::new(ReloadPipelinesResult { 
            pipelines_loaded: inner.pipelines_loaded 
        }))
    }
}