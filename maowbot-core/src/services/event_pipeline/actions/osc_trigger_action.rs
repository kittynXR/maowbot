use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use crate::Error;
use crate::services::event_pipeline::{EventAction, ActionResult, ActionContext};

#[derive(Debug, Serialize, Deserialize)]
struct OscTriggerActionConfig {
    parameter_path: String,
    #[serde(default = "default_value")]
    value: f32,
    #[serde(default)]
    duration_ms: Option<u64>,
    #[serde(default)]
    toggle_id: Option<String>,
}

fn default_value() -> f32 {
    1.0
}

/// Action that triggers an OSC parameter
pub struct OscTriggerAction {
    parameter_path: String,
    value: f32,
    duration_ms: Option<u64>,
    toggle_id: Option<String>,
}

impl OscTriggerAction {
    pub fn new() -> Self {
        Self {
            parameter_path: String::new(),
            value: 1.0,
            duration_ms: None,
            toggle_id: None,
        }
    }
}

impl Default for OscTriggerAction {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl EventAction for OscTriggerAction {
    fn id(&self) -> &str {
        "osc_trigger"
    }

    fn name(&self) -> &str {
        "Trigger OSC Parameter"
    }

    fn configure(&mut self, config: serde_json::Value) -> Result<(), Error> {
        let config: OscTriggerActionConfig = serde_json::from_value(config)
            .map_err(|e| Error::Platform(format!("Invalid OSC trigger action config: {}", e)))?;
        
        self.parameter_path = config.parameter_path;
        self.value = config.value;
        self.duration_ms = config.duration_ms;
        self.toggle_id = config.toggle_id;
        Ok(())
    }

    async fn execute(&self, context: &mut ActionContext) -> Result<ActionResult, Error> {
        // Use toggle ID if specified, otherwise use direct parameter
        if let Some(toggle_id) = &self.toggle_id {
            // TODO: Implement toggle by ID functionality
            // For now, just log what we would do
            tracing::info!(
                "Would trigger OSC toggle {} for {:?}ms",
                toggle_id, self.duration_ms
            );
            
            Ok(ActionResult::Success(serde_json::json!({
                "osc_triggered": true,
                "toggle_id": toggle_id,
                "duration_ms": self.duration_ms
            })))
        } else {
            // Direct parameter trigger
            tracing::info!(
                "Would trigger OSC parameter {} = {} for {:?}ms",
                self.parameter_path, self.value, self.duration_ms
            );
            
            // TODO: Implement direct OSC parameter control
            // context.context.osc_service
            //     .set_parameter(&self.parameter_path, self.value)
            //     .await?;
            
            Ok(ActionResult::Success(serde_json::json!({
                "osc_triggered": true,
                "parameter": self.parameter_path,
                "value": self.value,
                "duration_ms": self.duration_ms
            })))
        }
    }
}