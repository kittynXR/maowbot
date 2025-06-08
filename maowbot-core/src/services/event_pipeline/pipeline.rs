use std::sync::Arc;
use async_trait::async_trait;
use tracing::{debug, info, warn, error};
use crate::Error;
use crate::eventbus::BotEvent;
use crate::services::event_context::EventContext;
use crate::services::event_pipeline::{
    EventFilter, FilterResult,
    EventAction, ActionResult, ActionContext,
};

/// A complete event processing pipeline
pub struct EventPipeline {
    /// Unique identifier for this pipeline
    pub id: String,
    /// Human-readable name
    pub name: String,
    /// Whether this pipeline is enabled
    pub enabled: bool,
    /// Priority for execution order (lower numbers run first)
    pub priority: i32,
    /// Filters that must pass for the pipeline to execute
    pub filters: Vec<Box<dyn EventFilter>>,
    /// Actions to execute in order
    pub actions: Vec<Box<dyn EventAction>>,
    /// Whether to stop processing other pipelines if this one executes
    pub stop_on_match: bool,
}

impl EventPipeline {
    pub fn new(id: &str, name: &str) -> Self {
        Self {
            id: id.to_string(),
            name: name.to_string(),
            enabled: true,
            priority: 100,
            filters: Vec::new(),
            actions: Vec::new(),
            stop_on_match: false,
        }
    }

    /// Check if all filters pass for the given event
    pub async fn check_filters(&self, event: &BotEvent, context: &EventContext) -> Result<bool, Error> {
        for filter in &self.filters {
            match filter.apply(event, context).await? {
                FilterResult::Pass => {
                    debug!("Pipeline {}: Filter {} passed", self.id, filter.id());
                }
                FilterResult::Reject => {
                    debug!("Pipeline {}: Filter {} rejected event", self.id, filter.id());
                    return Ok(false);
                }
            }
        }
        Ok(true)
    }

    /// Execute all actions in the pipeline
    pub async fn execute_actions(
        &self,
        event: BotEvent,
        context: Arc<EventContext>,
    ) -> Result<bool, Error> {
        let mut action_context = ActionContext::new(event, context);
        let mut completed = false;

        for action in &self.actions {
            debug!("Pipeline {}: Executing action {}", self.id, action.id());
            
            match action.execute(&mut action_context).await {
                Ok(ActionResult::Success(data)) => {
                    debug!("Pipeline {}: Action {} completed successfully", self.id, action.id());
                    // Check if action requests stop
                    if let Some(stop) = data.get("stop").and_then(|v| v.as_bool()) {
                        if stop {
                            info!("Pipeline {}: Action {} requested stop", self.id, action.id());
                            completed = true;
                            break;
                        }
                    }
                }
                Ok(ActionResult::Error(msg)) => {
                    warn!("Pipeline {}: Action {} failed: {}", self.id, action.id(), msg);
                    // Continue with other actions unless configured to stop on error
                }
                Err(e) => {
                    error!("Pipeline {}: Action {} error: {:?}", self.id, action.id(), e);
                    return Err(e);
                }
            }
        }

        Ok(completed || self.stop_on_match)
    }
}

/// Manages and executes multiple pipelines
pub struct PipelineExecutor {
    pipelines: Vec<Arc<EventPipeline>>,
    context: Arc<EventContext>,
}

impl PipelineExecutor {
    pub fn new(context: Arc<EventContext>) -> Self {
        Self {
            pipelines: Vec::new(),
            context,
        }
    }

    /// Add a pipeline to the executor
    pub fn add_pipeline(&mut self, pipeline: EventPipeline) {
        let insert_pos = self.pipelines
            .binary_search_by_key(&pipeline.priority, |p| p.priority)
            .unwrap_or_else(|pos| pos);
        
        self.pipelines.insert(insert_pos, Arc::new(pipeline));
    }

    /// Remove a pipeline by ID
    pub fn remove_pipeline(&mut self, pipeline_id: &str) -> bool {
        if let Some(pos) = self.pipelines.iter().position(|p| p.id == pipeline_id) {
            self.pipelines.remove(pos);
            true
        } else {
            false
        }
    }

    /// Execute all applicable pipelines for an event
    pub async fn execute(&self, event: BotEvent) -> Result<(), Error> {
        info!("PipelineExecutor: Processing event with {} pipelines", self.pipelines.len());

        for pipeline in &self.pipelines {
            if !pipeline.enabled {
                debug!("Pipeline {} is disabled, skipping", pipeline.id);
                continue;
            }

            // Check filters
            match pipeline.check_filters(&event, &self.context).await {
                Ok(true) => {
                    info!("Pipeline {} passed all filters, executing actions", pipeline.id);
                }
                Ok(false) => {
                    debug!("Pipeline {} filtered out", pipeline.id);
                    continue;
                }
                Err(e) => {
                    error!("Pipeline {} filter error: {:?}", pipeline.id, e);
                    continue;
                }
            }

            // Execute actions
            match pipeline.execute_actions(event.clone(), self.context.clone()).await {
                Ok(stop) => {
                    if stop {
                        info!("Pipeline {} requested stop, halting further pipeline execution", pipeline.id);
                        break;
                    }
                }
                Err(e) => {
                    error!("Pipeline {} execution error: {:?}", pipeline.id, e);
                    // Continue with other pipelines unless this is a critical error
                }
            }
        }

        Ok(())
    }

    /// Get all registered pipelines
    pub fn pipelines(&self) -> &[Arc<EventPipeline>] {
        &self.pipelines
    }
    
    /// Get a pipeline by ID
    pub fn get_pipeline(&self, id: &str) -> Option<&Arc<EventPipeline>> {
        self.pipelines.iter().find(|p| p.id == id)
    }
}

/// Service that integrates pipelines with the event bus
pub struct PipelineEventService {
    event_bus: Arc<crate::eventbus::EventBus>,
    executor: Arc<tokio::sync::RwLock<PipelineExecutor>>,
}

impl PipelineEventService {
    pub fn new(
        event_bus: Arc<crate::eventbus::EventBus>,
        context: Arc<EventContext>,
    ) -> Self {
        Self {
            event_bus,
            executor: Arc::new(tokio::sync::RwLock::new(PipelineExecutor::new(context))),
        }
    }

    /// Add a pipeline
    pub async fn add_pipeline(&self, pipeline: EventPipeline) {
        let mut executor = self.executor.write().await;
        executor.add_pipeline(pipeline);
    }

    /// Remove a pipeline
    pub async fn remove_pipeline(&self, pipeline_id: &str) -> bool {
        let mut executor = self.executor.write().await;
        executor.remove_pipeline(pipeline_id)
    }

    /// Start listening to events and processing through pipelines
    pub async fn start(&self) {
        let mut rx = self.event_bus.subscribe(None).await;

        info!("PipelineEventService: Started, listening on EventBus");

        while let Some(event) = rx.recv().await {
            let executor = self.executor.read().await;
            
            if let Err(e) = executor.execute(event).await {
                error!("PipelineEventService: Error processing event: {:?}", e);
            }
        }

        info!("PipelineEventService: Shutting down");
    }
}