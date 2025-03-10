// File: maowbot-core/src/vrchat_osc/manager.rs
//! A high-level manager for the VRChat OSC subsystem. Spawns the OSC runtime,
//! orchestrates reading/writing of avatar configs, toggles, etc.

use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tracing::{info, error};

use crate::eventbus::EventBus;
use crate::vrchat_osc::runtime::VrchatOscRuntime;

/// A struct that owns or coordinates the VRChat OSC logic.
pub struct VrchatOscManager {
    pub event_bus: Arc<EventBus>,
    runtime_handle: Option<JoinHandle<()>>,
}

impl VrchatOscManager {
    /// Creates a new manager with references to the global EventBus, etc.
    pub fn new(event_bus: Arc<EventBus>) -> Self {
        Self {
            event_bus,
            runtime_handle: None,
        }
    }

    /// Spawns the OSC runtime task on port 9001 (by default) in a background Tokio task.
    pub async fn start(&mut self) -> Result<(), crate::Error> {
        if self.runtime_handle.is_some() {
            // Already started
            return Ok(());
        }

        // Create the runtime struct
        let mut runtime = VrchatOscRuntime::new(self.event_bus.clone());
        let handle = tokio::spawn(async move {
            if let Err(e) = runtime.run_main_loop().await {
                error!("VrchatOscRuntime ended with error: {:?}", e);
            }
        });
        self.runtime_handle = Some(handle);

        info!("VrchatOscManager started VRChat OSC runtime.");
        Ok(())
    }

    /// Graceful shutdown if needed.
    pub async fn stop(&mut self) {
        if let Some(handle) = self.runtime_handle.take() {
            handle.abort();
            info!("VrchatOscManager: VRChat OSC runtime aborted.");
        }
    }
}
