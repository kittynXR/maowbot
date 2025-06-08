use async_trait::async_trait;
use std::sync::Arc;
use std::any::Any;
use crate::Error;
use crate::eventbus::BotEvent;
use crate::services::event_context::EventContext;

/// Result of executing an action
#[derive(Debug, Clone)]
pub enum ActionResult {
    /// Action completed successfully with optional data
    Success(serde_json::Value),
    /// Action failed with error message
    Error(String),
}

/// Context passed between actions in a pipeline
pub struct ActionContext {
    /// The original event
    pub event: BotEvent,
    /// Shared event context with services
    pub context: Arc<EventContext>,
    /// Data shared between actions in the pipeline
    pub shared_data: std::collections::HashMap<String, serde_json::Value>,
    /// Execution ID for tracking
    pub execution_id: uuid::Uuid,
}

impl ActionContext {
    pub fn new(event: BotEvent, context: Arc<EventContext>) -> Self {
        Self {
            event,
            context,
            shared_data: std::collections::HashMap::new(),
            execution_id: uuid::Uuid::new_v4(),
        }
    }

    /// Store data for use by later actions
    pub fn set_data(&mut self, key: &str, value: serde_json::Value) {
        self.shared_data.insert(key.to_string(), value);
    }

    /// Retrieve data stored by previous actions
    pub fn get_data(&self, key: &str) -> Option<&serde_json::Value> {
        self.shared_data.get(key)
    }
}

/// Trait for pipeline actions
#[async_trait]
pub trait EventAction: Send + Sync {
    /// Unique identifier for this action
    fn id(&self) -> &str;
    
    /// Human-readable name for this action
    fn name(&self) -> &str;
    
    /// Configure the action from JSON configuration
    fn configure(&mut self, config: serde_json::Value) -> Result<(), Error> {
        // Default implementation does nothing
        Ok(())
    }
    
    /// Execute the action
    async fn execute(&self, context: &mut ActionContext) -> Result<ActionResult, Error>;
    
    /// Whether this action can be run in parallel with others
    fn is_parallelizable(&self) -> bool {
        false
    }
}

// Built-in actions

/// Action that logs event details
pub struct LogAction {
    level: tracing::Level,
}

impl LogAction {
    pub fn new(level: tracing::Level) -> Self {
        Self { level }
    }
}

#[async_trait]
impl EventAction for LogAction {
    fn id(&self) -> &str {
        "builtin.log"
    }

    fn name(&self) -> &str {
        "Log Event"
    }

    async fn execute(&self, context: &mut ActionContext) -> Result<ActionResult, Error> {
        match self.level {
            tracing::Level::ERROR => tracing::error!("Event Pipeline: {:?}", context.event),
            tracing::Level::WARN => tracing::warn!("Event Pipeline: {:?}", context.event),
            tracing::Level::INFO => tracing::info!("Event Pipeline: {:?}", context.event),
            tracing::Level::DEBUG => tracing::debug!("Event Pipeline: {:?}", context.event),
            tracing::Level::TRACE => tracing::trace!("Event Pipeline: {:?}", context.event),
        }
        Ok(ActionResult::Success(serde_json::json!({"logged": true})))
    }

    fn is_parallelizable(&self) -> bool {
        true
    }
}

/// Action that sends a Discord message
pub struct DiscordMessageAction {
    account: String,
    channel_id: String,
    message_template: String,
}

impl DiscordMessageAction {
    pub fn new(account: &str, channel_id: &str, message_template: &str) -> Self {
        Self {
            account: account.to_string(),
            channel_id: channel_id.to_string(),
            message_template: message_template.to_string(),
        }
    }
    
    fn format_message(&self, context: &ActionContext) -> String {
        // Simple template replacement - could be enhanced with proper templating
        let mut message = self.message_template.clone();
        
        match &context.event {
            BotEvent::ChatMessage { user, text, .. } => {
                message = message.replace("{user}", user);
                message = message.replace("{message}", text);
            }
            _ => {}
        }
        
        message
    }
}

#[async_trait]
impl EventAction for DiscordMessageAction {
    fn id(&self) -> &str {
        "builtin.discord_message"
    }

    fn name(&self) -> &str {
        "Send Discord Message"
    }

    async fn execute(&self, context: &mut ActionContext) -> Result<ActionResult, Error> {
        let message = self.format_message(context);
        
        // Get guild ID from context if available
        let guild_id = context.get_data("guild_id")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        
        context.context.platform_manager
            .send_discord_message(&self.account, guild_id, &self.channel_id, &message)
            .await?;
        
        Ok(ActionResult::Success(serde_json::json!({})))
    }
}

/// Action that triggers an OSC parameter
pub struct OSCTriggerAction {
    parameter_path: String,
    value: f32,
    duration_ms: Option<u64>,
}

impl OSCTriggerAction {
    pub fn new(parameter_path: &str, value: f32, duration_ms: Option<u64>) -> Self {
        Self {
            parameter_path: parameter_path.to_string(),
            value,
            duration_ms,
        }
    }
}

#[async_trait]
impl EventAction for OSCTriggerAction {
    fn id(&self) -> &str {
        "builtin.osc_trigger"
    }

    fn name(&self) -> &str {
        "Trigger OSC Parameter"
    }

    async fn execute(&self, context: &mut ActionContext) -> Result<ActionResult, Error> {
        // This would use the OSC toggle service to trigger the parameter
        tracing::info!(
            "OSCTriggerAction: Would trigger {} = {} for {:?}ms",
            self.parameter_path, self.value, self.duration_ms
        );
        
        // context.services.osc_toggle_service.trigger_parameter(
        //     &self.parameter_path,
        //     self.value,
        //     self.duration_ms,
        // ).await?;
        
        Ok(ActionResult::Success(serde_json::json!({})))
    }
}

/// Action that executes a plugin function
pub struct PluginAction {
    plugin_id: String,
    function_name: String,
    parameters: std::collections::HashMap<String, String>,
}

impl PluginAction {
    pub fn new(plugin_id: &str, function_name: &str) -> Self {
        Self {
            plugin_id: plugin_id.to_string(),
            function_name: function_name.to_string(),
            parameters: std::collections::HashMap::new(),
        }
    }
    
    pub fn with_parameter(mut self, key: &str, value: &str) -> Self {
        self.parameters.insert(key.to_string(), value.to_string());
        self
    }
}

#[async_trait]
impl EventAction for PluginAction {
    fn id(&self) -> &str {
        "builtin.plugin"
    }

    fn name(&self) -> &str {
        "Execute Plugin Function"
    }

    async fn execute(&self, context: &mut ActionContext) -> Result<ActionResult, Error> {
        tracing::info!(
            "PluginAction: Would execute {}.{} with params {:?}",
            self.plugin_id, self.function_name, self.parameters
        );
        
        // This would call the plugin manager to execute the function
        // let result = context.services.plugin_manager
        //     .execute_function(&self.plugin_id, &self.function_name, self.parameters.clone())
        //     .await?;
        
        Ok(ActionResult::Success(serde_json::json!({})))
    }
}