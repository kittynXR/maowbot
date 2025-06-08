use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use crate::Error;
use crate::services::event_pipeline::{EventAction, ActionResult, ActionContext};

#[derive(Debug, Serialize, Deserialize)]
struct PluginCallActionConfig {
    plugin_id: String,
    function_name: String,
    #[serde(default)]
    parameters: HashMap<String, serde_json::Value>,
    #[serde(default)]
    pass_event: bool,
}

/// Action that executes a plugin function
pub struct PluginCallAction {
    plugin_id: String,
    function_name: String,
    parameters: HashMap<String, serde_json::Value>,
    pass_event: bool,
}

impl PluginCallAction {
    pub fn new() -> Self {
        Self {
            plugin_id: String::new(),
            function_name: String::new(),
            parameters: HashMap::new(),
            pass_event: true,
        }
    }
    
    fn prepare_parameters(&self, context: &ActionContext) -> HashMap<String, String> {
        let mut params = HashMap::new();
        
        // Convert configured parameters
        for (key, value) in &self.parameters {
            let str_value = match value {
                serde_json::Value::String(s) => {
                    // Check for placeholders
                    let mut s = s.clone();
                    
                    // Replace event placeholders
                    match &context.event {
                        crate::eventbus::BotEvent::ChatMessage { platform, channel, user, text, .. } => {
                            s = s.replace("{platform}", platform);
                            s = s.replace("{channel}", channel);
                            s = s.replace("{user}", user);
                            s = s.replace("{message}", text);
                            s = s.replace("{text}", text);
                        }
                        _ => {}
                    }
                    
                    // Replace shared data placeholders
                    for (data_key, data_value) in &context.shared_data {
                        if let Some(str_val) = data_value.as_str() {
                            s = s.replace(&format!("{{{}}}", data_key), str_val);
                        }
                    }
                    
                    s
                }
                _ => value.to_string(),
            };
            
            params.insert(key.clone(), str_value);
        }
        
        // Add event data if requested
        if self.pass_event {
            params.insert("event_type".to_string(), context.event.event_type());
            // Can't serialize entire event due to TwitchEventSub types
            match &context.event {
                crate::eventbus::BotEvent::ChatMessage { platform, channel, user, text, timestamp, metadata: _ } => {
                    params.insert("event_platform".to_string(), platform.clone());
                    params.insert("event_channel".to_string(), channel.clone());
                    params.insert("event_user".to_string(), user.clone());
                    params.insert("event_text".to_string(), text.clone());
                    params.insert("event_timestamp".to_string(), timestamp.to_rfc3339());
                }
                _ => {
                    params.insert("event_data".to_string(), format!("{:?}", context.event));
                }
            }
        }
        
        params
    }
}

impl Default for PluginCallAction {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl EventAction for PluginCallAction {
    fn id(&self) -> &str {
        "plugin_call"
    }

    fn name(&self) -> &str {
        "Execute Plugin Function"
    }

    fn configure(&mut self, config: serde_json::Value) -> Result<(), Error> {
        let config: PluginCallActionConfig = serde_json::from_value(config)
            .map_err(|e| Error::Platform(format!("Invalid plugin call action config: {}", e)))?;
        
        self.plugin_id = config.plugin_id;
        self.function_name = config.function_name;
        self.parameters = config.parameters;
        self.pass_event = config.pass_event;
        Ok(())
    }

    async fn execute(&self, context: &mut ActionContext) -> Result<ActionResult, Error> {
        let params = self.prepare_parameters(context);
        
        // Call plugin function
        // TODO: Add plugin_manager to EventContext
        let result: Result<String, Error> = Err(Error::Internal("Plugin manager not available in EventContext".to_string()));
        
        match result {
            Ok(response) => {
                // Store plugin response in shared data
                if !response.is_empty() {
                    context.set_data("plugin_response", serde_json::Value::String(response.clone()));
                }
                
                Ok(ActionResult::Success(serde_json::json!({
                    "plugin_called": true,
                    "plugin_id": self.plugin_id,
                    "function": self.function_name,
                    "parameters": params,
                    "response": response
                })))
            }
            Err(e) => {
                Ok(ActionResult::Error(format!("Plugin call failed: {:?}", e)))
            }
        }
    }
}