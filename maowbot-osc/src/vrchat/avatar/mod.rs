//! maowbot-osc/src/vrchat/avatar/mod.rs
//!
//! For code specifically managing avatar-level toggles,
//! saving parameter changes, etc.

use crate::Result;
use crate::vrchat::VrchatAvatarConfig;

/// A stub object or manager that might hold an in-memory representation
/// of the currently used avatar toggles/params.
pub struct AvatarManager {
    pub config: VrchatAvatarConfig,
}

impl AvatarManager {
    pub fn new(config: VrchatAvatarConfig) -> Self {
        Self { config }
    }

    /// Example method for toggling a named parameter (internal only).
    /// The actual OSC message sending is left to your main module
    /// or the `send_osc_toggle` method in `MaowOscManager`.
    pub fn toggle_parameter(&mut self, param_name: &str, new_value: bool) -> Result<()> {
        // find param in self.config.parameters
        // update if needed ...
        Ok(())
    }
}
