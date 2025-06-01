//! maowbot-osc/src/vrchat/avatar/mod.rs
//!
//! For code specifically managing avatar-level toggles,
//! saving parameter changes, etc.

use crate::Result;
use crate::vrchat::VrchatAvatarConfig;
use serde::{Deserialize, Serialize};

/// Represent a parameter type so we can store or manipulate it in memory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AvatarParameterValue {
    Bool(bool),
    Int(i32),
    Float(f32),
}

/// A stub object or manager that might hold an in-memory representation
/// of the currently used avatar toggles/params.
pub struct AvatarManager {
    pub config: VrchatAvatarConfig,
    // Possibly store the "current" parameter values locally if we wish.
    pub local_params: Vec<(String, AvatarParameterValue)>,
}

impl AvatarManager {
    pub fn new(config: VrchatAvatarConfig) -> Self {
        let local_params = config
            .parameters
            .iter()
            .map(|p| {
                // Default everything to false or 0.0
                // for demonstration. Real usage might vary.
                (p.name.clone(), AvatarParameterValue::Bool(false))
            })
            .collect();

        Self {
            config,
            local_params,
        }
    }

    /// Set a parameter in the local manager. Does NOT send OSC.
    pub fn set_parameter_local(&mut self, param_name: &str, value: AvatarParameterValue) -> Result<()> {
        if let Some(entry) = self.local_params.iter_mut().find(|(n, _)| n == param_name) {
            entry.1 = value;
        } else {
            // If not found in our known list, we can optionally push it or ignore.
            self.local_params.push((param_name.to_string(), value));
        }
        Ok(())
    }

    /// Example method for toggling a named parameter locally if it's bool type.
    pub fn toggle_parameter_local(&mut self, param_name: &str) -> Result<()> {
        if let Some(entry) = self.local_params.iter_mut().find(|(n, _)| n == param_name) {
            if let AvatarParameterValue::Bool(b) = entry.1 {
                entry.1 = AvatarParameterValue::Bool(!b);
            }
        }
        Ok(())
    }

    /// Retrieve the local parameter value if stored.
    pub fn get_parameter_local(&self, param_name: &str) -> Option<&AvatarParameterValue> {
        self.local_params.iter().find_map(|(n, v)| {
            if n == param_name {
                Some(v)
            } else {
                None
            }
        })
    }
}
