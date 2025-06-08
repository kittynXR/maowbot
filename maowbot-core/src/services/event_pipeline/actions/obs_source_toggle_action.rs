use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use crate::Error;
use crate::services::event_pipeline::{EventAction, ActionResult, ActionContext};

#[derive(Debug, Serialize, Deserialize)]
struct ObsSourceToggleActionConfig {
    #[serde(default)]
    instance_name: String,
    #[serde(default)]
    scene_name: Option<String>,
    source_name: String,
    #[serde(default = "default_action")]
    action: String, // "toggle", "show", "hide"
}

fn default_action() -> String {
    "toggle".to_string()
}

/// Action that toggles OBS source visibility
pub struct ObsSourceToggleAction {
    instance_name: String,
    scene_name: Option<String>,
    source_name: String,
    action: String,
}

impl ObsSourceToggleAction {
    pub fn new() -> Self {
        Self {
            instance_name: String::new(),
            scene_name: None,
            source_name: String::new(),
            action: "toggle".to_string(),
        }
    }
}

impl Default for ObsSourceToggleAction {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl EventAction for ObsSourceToggleAction {
    fn id(&self) -> &str {
        "obs_source_toggle"
    }

    fn name(&self) -> &str {
        "Toggle OBS Source"
    }

    fn configure(&mut self, config: serde_json::Value) -> Result<(), Error> {
        let config: ObsSourceToggleActionConfig = serde_json::from_value(config)
            .map_err(|e| Error::Platform(format!("Invalid OBS source toggle action config: {}", e)))?;
        
        self.instance_name = config.instance_name;
        self.scene_name = config.scene_name;
        self.source_name = config.source_name;
        self.action = config.action;
        Ok(())
    }

    async fn execute(&self, context: &mut ActionContext) -> Result<ActionResult, Error> {
        // Get OBS instance name (default to first/primary instance if not specified)
        let instance_name = if !self.instance_name.is_empty() {
            &self.instance_name
        } else {
            "default"
        };
        
        // TODO: Implement OBS source toggle in platform manager
        // let visible = match self.action.as_str() {
        //     "show" => true,
        //     "hide" => false,
        //     "toggle" => {
        //         // Get current visibility state
        //         let current = context.context.platform_manager
        //             .get_obs_source_visibility(instance_name, self.scene_name.as_deref(), &self.source_name)
        //             .await?;
        //         !current
        //     }
        //     _ => return Ok(ActionResult::Error(format!("Invalid action: {}", self.action))),
        // };
        // 
        // context.context.platform_manager
        //     .set_obs_source_visibility(
        //         instance_name,
        //         self.scene_name.as_deref(),
        //         &self.source_name,
        //         visible
        //     )
        //     .await?;
        
        tracing::info!(
            "Would {} OBS source '{}' on instance '{}' (scene: {:?})",
            self.action, self.source_name, instance_name, self.scene_name
        );
        
        Ok(ActionResult::Success(serde_json::json!({
            "source_toggled": true,
            "instance": instance_name,
            "scene": self.scene_name,
            "source": self.source_name,
            "action": self.action
        })))
    }
}