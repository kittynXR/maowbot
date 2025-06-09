use std::sync::Arc;
use std::collections::HashMap;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn, trace};
use chrono::Utc;
use uuid::Uuid;

use crate::Error;
use crate::eventbus::{EventBus, BotEvent};
use crate::services::event_context::EventContext;
use crate::repositories::postgres::event_pipeline::PostgresEventPipelineRepository;

use maowbot_common::models::event_pipeline::{
    EventPipeline as DbPipeline, PipelineFilter as DbFilter, PipelineAction as DbAction,
    PipelineExecutionLog, PipelineExecutionStatus, ActionExecutionResult, ActionExecutionStatus,
};
use maowbot_common::traits::event_pipeline_traits::{
    EventPipelineRepository, PipelineExecutionLogRepository, PipelineSharedDataRepository,
    EventTypeRegistryRepository, EventHandlerRegistryRepository,
};

// Import our filter and action traits
use super::event_pipeline::{EventFilter, FilterResult, EventAction, ActionResult, ActionContext};

// Import built-in implementations (to be created)
use super::event_pipeline::filters::*;
use super::event_pipeline::actions::*;

/// Service that manages and executes database-driven event pipelines
pub struct EventPipelineService {
    event_bus: Arc<EventBus>,
    context: Arc<EventContext>,
    pub repository: Arc<PostgresEventPipelineRepository>,
    
    // Cache of loaded pipelines
    pub pipelines: Arc<RwLock<Vec<LoadedPipeline>>>,
    
    // Registry of available filter/action types
    filter_registry: Arc<RwLock<HashMap<String, Box<dyn Fn() -> Box<dyn EventFilter> + Send + Sync>>>>,
    action_registry: Arc<RwLock<HashMap<String, Box<dyn Fn() -> Box<dyn EventAction> + Send + Sync>>>>,
}

/// A pipeline loaded from the database with instantiated filters and actions
struct LoadedPipeline {
    pub pipeline: DbPipeline,
    pub filters: Vec<(DbFilter, Box<dyn EventFilter>)>,
    pub actions: Vec<(DbAction, Box<dyn EventAction>)>,
}

impl EventPipelineService {
    /// Get the count of loaded pipelines
    pub async fn pipeline_count(&self) -> usize {
        self.pipelines.read().await.len()
    }
    
    pub async fn new(
        event_bus: Arc<EventBus>,
        context: Arc<EventContext>,
        repository: Arc<PostgresEventPipelineRepository>,
    ) -> Result<Self, Error> {
        let service = Self {
            event_bus,
            context,
            repository,
            pipelines: Arc::new(RwLock::new(Vec::new())),
            filter_registry: Arc::new(RwLock::new(HashMap::new())),
            action_registry: Arc::new(RwLock::new(HashMap::new())),
        };
        
        // Register built-in filters and actions
        service.register_builtin_handlers().await?;
        
        // Load pipelines from database
        service.reload_pipelines().await?;
        
        Ok(service)
    }
    
    /// Register all built-in filter and action types
    async fn register_builtin_handlers(&self) -> Result<(), Error> {
        let mut filters = self.filter_registry.write().await;
        let mut actions = self.action_registry.write().await;
        
        // Register filters
        filters.insert("platform_filter".to_string(), 
            Box::new(|| Box::new(PlatformFilter::new(vec![])) as Box<dyn EventFilter>));
        filters.insert("channel_filter".to_string(),
            Box::new(|| Box::new(ChannelFilter::new(vec![])) as Box<dyn EventFilter>));
        filters.insert("user_role_filter".to_string(),
            Box::new(|| Box::new(UserRoleFilter::new(vec![], true)) as Box<dyn EventFilter>));
        filters.insert("user_level_filter".to_string(),
            Box::new(|| Box::new(UserLevelFilter::new("viewer".to_string())) as Box<dyn EventFilter>));
        filters.insert("message_pattern_filter".to_string(),
            Box::new(|| Box::new(MessagePatternFilter::new(vec![], true).unwrap()) as Box<dyn EventFilter>));
        filters.insert("message_length_filter".to_string(),
            Box::new(|| Box::new(MessageLengthFilter::new(0, 500)) as Box<dyn EventFilter>));
        filters.insert("time_window_filter".to_string(),
            Box::new(|| Box::new(TimeWindowFilter::new(0, 23, "UTC".to_string())) as Box<dyn EventFilter>));
        filters.insert("cooldown_filter".to_string(),
            Box::new(|| Box::new(CooldownFilter::new(60, true)) as Box<dyn EventFilter>));
        
        // Register actions
        actions.insert("log_action".to_string(),
            Box::new(|| Box::new(LogAction::new("info".to_string())) as Box<dyn EventAction>));
        actions.insert("discord_message".to_string(),
            Box::new(|| Box::new(DiscordMessageAction::new()) as Box<dyn EventAction>));
        actions.insert("discord_role_add".to_string(),
            Box::new(|| Box::new(DiscordRoleAddAction::new()) as Box<dyn EventAction>));
        actions.insert("discord_role_remove".to_string(),
            Box::new(|| Box::new(DiscordRoleRemoveAction::new()) as Box<dyn EventAction>));
        actions.insert("twitch_message".to_string(),
            Box::new(|| Box::new(TwitchMessageAction::new()) as Box<dyn EventAction>));
        actions.insert("twitch_timeout".to_string(),
            Box::new(|| Box::new(TwitchTimeoutAction::new()) as Box<dyn EventAction>));
        actions.insert("osc_trigger".to_string(),
            Box::new(|| Box::new(OscTriggerAction::new()) as Box<dyn EventAction>));
        actions.insert("obs_scene_change".to_string(),
            Box::new(|| Box::new(ObsSceneChangeAction::new()) as Box<dyn EventAction>));
        actions.insert("obs_source_toggle".to_string(),
            Box::new(|| Box::new(ObsSourceToggleAction::new()) as Box<dyn EventAction>));
        actions.insert("plugin_call".to_string(),
            Box::new(|| Box::new(PluginCallAction::new()) as Box<dyn EventAction>));
        actions.insert("ai_respond".to_string(),
            Box::new(|| Box::new(AiRespondAction::new()) as Box<dyn EventAction>));
        
        info!("Registered {} built-in filters and {} built-in actions", 
              filters.len(), actions.len());
        
        Ok(())
    }
    
    /// Load/reload all pipelines from the database
    pub async fn reload_pipelines(&self) -> Result<(), Error> {
        info!("Loading pipelines from database...");
        
        let db_pipelines = self.repository.list_pipelines(true).await?;
        let mut loaded_pipelines = Vec::new();
        
        for pipeline in db_pipelines {
            match self.load_pipeline(&pipeline).await {
                Ok(loaded) => loaded_pipelines.push(loaded),
                Err(e) => error!("Failed to load pipeline {}: {:?}", pipeline.name, e),
            }
        }
        
        // Sort by priority (lower numbers first)
        loaded_pipelines.sort_by_key(|p| p.pipeline.priority);
        
        let count = loaded_pipelines.len();
        *self.pipelines.write().await = loaded_pipelines;
        
        info!("Loaded {} pipelines from database", count);
        Ok(())
    }
    
    /// Load a single pipeline with its filters and actions
    async fn load_pipeline(&self, pipeline: &DbPipeline) -> Result<LoadedPipeline, Error> {
        let pipeline_id = pipeline.pipeline_id;
        
        // Load filters
        let db_filters = self.repository.list_filters_for_pipeline(pipeline_id).await?;
        let mut filters = Vec::new();
        
        for db_filter in db_filters {
            match self.instantiate_filter(&db_filter).await {
                Ok(filter) => filters.push((db_filter, filter)),
                Err(e) => {
                    error!("Failed to instantiate filter {} for pipeline {}: {:?}", 
                           db_filter.filter_type, pipeline.name, e);
                    return Err(e);
                }
            }
        }
        
        // Load actions
        let db_actions = self.repository.list_actions_for_pipeline(pipeline_id).await?;
        let mut actions = Vec::new();
        
        for db_action in db_actions {
            match self.instantiate_action(&db_action).await {
                Ok(action) => actions.push((db_action, action)),
                Err(e) => {
                    error!("Failed to instantiate action {} for pipeline {}: {:?}", 
                           db_action.action_type, pipeline.name, e);
                    return Err(e);
                }
            }
        }
        
        Ok(LoadedPipeline {
            pipeline: pipeline.clone(),
            filters,
            actions,
        })
    }
    
    /// Create a filter instance from database configuration
    async fn instantiate_filter(&self, db_filter: &DbFilter) -> Result<Box<dyn EventFilter>, Error> {
        let registry = self.filter_registry.read().await;
        
        let factory = registry.get(&db_filter.filter_type)
            .ok_or_else(|| Error::NotFound(format!("Unknown filter type: {}", db_filter.filter_type)))?;
        
        let mut filter = factory();
        filter.configure(db_filter.filter_config.clone())?;
        
        Ok(filter)
    }
    
    /// Create an action instance from database configuration
    async fn instantiate_action(&self, db_action: &DbAction) -> Result<Box<dyn EventAction>, Error> {
        let registry = self.action_registry.read().await;
        
        let factory = registry.get(&db_action.action_type)
            .ok_or_else(|| Error::NotFound(format!("Unknown action type: {}", db_action.action_type)))?;
        
        let mut action = factory();
        action.configure(db_action.action_config.clone())?;
        
        Ok(action)
    }
    
    /// Start listening for events on the event bus
    pub async fn start(&self) {
        let mut rx = self.event_bus.subscribe(None).await;
        info!("EventPipelineService started, listening on EventBus");
        
        while let Some(event) = rx.recv().await {
            // Clone what we need for the spawned task
            let pipelines = self.pipelines.clone();
            let context = self.context.clone();
            let repository = self.repository.clone();
            
            // Process event in a separate task to avoid blocking
            tokio::spawn(async move {
                if let Err(e) = Self::process_event(event, pipelines, context, repository).await {
                    error!("Error processing event through pipelines: {:?}", e);
                }
            });
        }
    }
    
    /// Process an event through all matching pipelines
    async fn process_event(
        event: BotEvent,
        pipelines: Arc<RwLock<Vec<LoadedPipeline>>>,
        context: Arc<EventContext>,
        repository: Arc<PostgresEventPipelineRepository>,
    ) -> Result<(), Error> {
        let event_type = event.event_type();
        let platform = event.platform().map(|p| p.to_string()).unwrap_or_default();
        
        trace!("Processing event {} from platform {} through pipelines", event_type, platform);
        
        let pipelines = pipelines.read().await;
        
        for loaded_pipeline in pipelines.iter() {
            if !loaded_pipeline.pipeline.enabled {
                continue;
            }
            
            // Create execution log
            let execution_id = match repository.create_execution(
                loaded_pipeline.pipeline.pipeline_id,
                &event_type,
                serde_json::json!({
                    "event_type": event_type,
                    "platform": platform
                })
            ).await {
                Ok(log) => log.execution_id,
                Err(e) => {
                    error!("Failed to create execution log: {:?}", e);
                    continue;
                }
            };
            
            // Check filters
            let mut all_filters_pass = true;
            for (db_filter, filter) in &loaded_pipeline.filters {
                match filter.apply(&event, &context).await {
                    Ok(FilterResult::Pass) => {
                        trace!("Pipeline {}: Filter {} passed", 
                               loaded_pipeline.pipeline.name, db_filter.filter_type);
                    }
                    Ok(FilterResult::Reject) => {
                        trace!("Pipeline {}: Filter {} rejected", 
                               loaded_pipeline.pipeline.name, db_filter.filter_type);
                        all_filters_pass = false;
                        break;
                    }
                    Err(e) => {
                        error!("Pipeline {}: Filter {} error: {:?}", 
                               loaded_pipeline.pipeline.name, db_filter.filter_type, e);
                        all_filters_pass = false;
                        break;
                    }
                }
            }
            
            if !all_filters_pass {
                // Update execution as skipped
                let _ = repository.update_execution_status(
                    execution_id,
                    PipelineExecutionStatus::Success,
                    Some("Filters did not match".to_string())
                ).await;
                continue;
            }
            
            info!("Executing pipeline {} for event {}", loaded_pipeline.pipeline.name, event_type);
            
            // Execute actions
            let mut action_context = ActionContext {
                event: event.clone(),
                context: context.clone(),
                shared_data: HashMap::new(),
                execution_id,
            };
            
            let mut any_failed = false;
            for (db_action, action) in &loaded_pipeline.actions {
                let action_start = Utc::now();
                
                match action.execute(&mut action_context).await {
                    Ok(ActionResult::Success(data)) => {
                        trace!("Pipeline {}: Action {} succeeded", 
                               loaded_pipeline.pipeline.name, db_action.action_type);
                        
                        // Record success
                        let _ = repository.add_action_result(
                            execution_id,
                            serde_json::json!({
                                "action_id": db_action.action_id,
                                "action_type": db_action.action_type,
                                "status": "success",
                                "started_at": action_start,
                                "completed_at": Utc::now(),
                                "output": data,
                            })
                        ).await;
                    }
                    Ok(ActionResult::Error(msg)) => {
                        error!("Pipeline {}: Action {} failed: {}", 
                               loaded_pipeline.pipeline.name, db_action.action_type, msg);
                        
                        // Record failure
                        let _ = repository.add_action_result(
                            execution_id,
                            serde_json::json!({
                                "action_id": db_action.action_id,
                                "action_type": db_action.action_type,
                                "status": "failed",
                                "started_at": action_start,
                                "completed_at": Utc::now(),
                                "error": msg,
                            })
                        ).await;
                        
                        if !db_action.continue_on_error {
                            any_failed = true;
                            break;
                        }
                    }
                    Err(e) => {
                        error!("Pipeline {}: Action {} error: {:?}", 
                               loaded_pipeline.pipeline.name, db_action.action_type, e);
                        
                        // Record error
                        let _ = repository.add_action_result(
                            execution_id,
                            serde_json::json!({
                                "action_id": db_action.action_id,
                                "action_type": db_action.action_type,
                                "status": "failed",
                                "started_at": action_start,
                                "completed_at": Utc::now(),
                                "error": format!("{:?}", e),
                            })
                        ).await;
                        
                        if !db_action.continue_on_error {
                            any_failed = true;
                            break;
                        }
                    }
                }
            }
            
            // Update execution status
            let status = if any_failed {
                PipelineExecutionStatus::Failed
            } else {
                PipelineExecutionStatus::Success
            };
            
            let _ = repository.update_execution_status(execution_id, status, None).await;
            
            // Update pipeline stats
            let _ = repository.increment_execution_stats(
                loaded_pipeline.pipeline.pipeline_id,
                !any_failed
            ).await;
            
            // Check if we should stop processing other pipelines
            if loaded_pipeline.pipeline.stop_on_match && !any_failed {
                info!("Pipeline {} executed with stop_on_match, skipping remaining pipelines", 
                      loaded_pipeline.pipeline.name);
                break;
            }
        }
        
        Ok(())
    }
    
    /// Register a custom filter type (for plugins)
    pub async fn register_filter_type<F>(&self, name: String, factory: F) -> Result<(), Error>
    where
        F: Fn() -> Box<dyn EventFilter> + Send + Sync + 'static,
    {
        let mut registry = self.filter_registry.write().await;
        if registry.contains_key(&name) {
            return Err(Error::Platform(format!("Filter type {} already registered", name)));
        }
        registry.insert(name.clone(), Box::new(factory));
        info!("Registered custom filter type: {}", name);
        Ok(())
    }
    
    /// Register a custom action type (for plugins)
    pub async fn register_action_type<F>(&self, name: String, factory: F) -> Result<(), Error>
    where
        F: Fn() -> Box<dyn EventAction> + Send + Sync + 'static,
    {
        let mut registry = self.action_registry.write().await;
        if registry.contains_key(&name) {
            return Err(Error::Platform(format!("Action type {} already registered", name)));
        }
        registry.insert(name.clone(), Box::new(factory));
        info!("Registered custom action type: {}", name);
        Ok(())
    }
}
