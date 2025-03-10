//! maowbot-osc/src/vrchat/toggles/mod.rs
//!
//! Specific logic for "simple toggles" that can be turned on/off.
//! These might be subsets of the avatar parameters or
//! separate user-defined toggles.

use crate::Result;

#[derive(Debug)]
pub struct SimpleToggle {
    pub name: String,
    pub is_on: bool,
}

impl SimpleToggle {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.into(),
            is_on: false,
        }
    }

    pub fn set(&mut self, on: bool) {
        self.is_on = on;
    }

    /// Flip the toggle from on->off or off->on
    pub fn toggle(&mut self) {
        self.is_on = !self.is_on;
    }
}

/// Possibly a manager for multiple toggles
pub struct ToggleManager {
    toggles: Vec<SimpleToggle>,
}

impl ToggleManager {
    pub fn new() -> Self {
        Self { toggles: vec![] }
    }

    pub fn add_toggle(&mut self, name: &str) {
        self.toggles.push(SimpleToggle::new(name));
    }

    pub fn set_toggle(&mut self, name: &str, on: bool) -> Result<()> {
        if let Some(t) = self.toggles.iter_mut().find(|t| t.name == name) {
            t.set(on);
        }
        Ok(())
    }

    pub fn toggle(&mut self, name: &str) -> Result<()> {
        if let Some(t) = self.toggles.iter_mut().find(|t| t.name == name) {
            t.toggle();
        }
        Ok(())
    }

    pub fn get_toggle(&self, name: &str) -> Option<bool> {
        self.toggles.iter().find_map(|t| {
            if t.name == name {
                Some(t.is_on)
            } else {
                None
            }
        })
    }
}
