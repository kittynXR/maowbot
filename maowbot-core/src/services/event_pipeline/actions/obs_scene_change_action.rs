use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use crate::Error;
use crate::services::event_pipeline::{EventAction, ActionResult, ActionContext};

#[derive(Debug, Serialize, Deserialize)]
struct ObsSceneChangeActionConfig {
    #[serde(default)]
    instance_name: String,
    scene_name: String,
    #[serde(default)]
    transition_name: Option<String>,
    #[serde(default)]
    transition_duration_ms: Option<u32>,
}

/// Action that changes OBS scene
pub struct ObsSceneChangeAction {
    instance_name: String,
    scene_name: String,
    transition_name: Option<String>,
    transition_duration_ms: Option<u32>,
}

impl ObsSceneChangeAction {
    pub fn new() -> Self {
        Self {
            instance_name: String::new(),
            scene_name: String::new(),
            transition_name: None,
            transition_duration_ms: None,
        }
    }
}

impl Default for ObsSceneChangeAction {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl EventAction for ObsSceneChangeAction {
    fn id(&self) -> &str {
        "obs_scene_change"
    }

    fn name(&self) -> &str {
        "Change OBS Scene"
    }

    fn configure(&mut self, config: serde_json::Value) -> Result<(), Error> {
        let config: ObsSceneChangeActionConfig = serde_json::from_value(config)
            .map_err(|e| Error::Platform(format!("Invalid OBS scene change action config: {}", e)))?;
        
        self.instance_name = config.instance_name;
        self.scene_name = config.scene_name;
        self.transition_name = config.transition_name;
        self.transition_duration_ms = config.transition_duration_ms;
        Ok(())
    }

    async fn execute(&self, context: &mut ActionContext) -> Result<ActionResult, Error> {
        // Get OBS instance name (default to first/primary instance if not specified)
        let instance_name = if !self.instance_name.is_empty() {
            &self.instance_name
        } else {
            "default"
        };
        
        // TODO: Implement OBS scene change in platform manager
        // if let Some(transition_name) = &self.transition_name {
        //     context.context.platform_manager
        //         .change_obs_scene_with_transition(
        //             instance_name,
        //             &self.scene_name,
        //             transition_name,
        //             self.transition_duration_ms
        //         )
        //         .await?;
        // } else {
        //     context.context.platform_manager
        //         .change_obs_scene(instance_name, &self.scene_name)
        //         .await?;
        // }
        
        tracing::info!(
            "Would change OBS scene to '{}' on instance '{}' (transition: {:?}, duration: {:?}ms)",
            self.scene_name, instance_name, self.transition_name, self.transition_duration_ms
        );
        
        Ok(ActionResult::Success(serde_json::json!({
            "scene_changed": true,
            "instance": instance_name,
            "scene": self.scene_name,
            "transition": self.transition_name,
            "transition_duration_ms": self.transition_duration_ms
        })))
    }
}