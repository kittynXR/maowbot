// File: maowbot-osc/src/vrchat/toggles/avatar_toggle_menu.rs

use crate::vrchat::VrchatAvatarConfig;

/// A simple “menu” object that you might expand to show toggles in a UI.
pub struct AvatarToggleMenu {
    avatar_name: String,
    parameters: Vec<String>,
}

impl AvatarToggleMenu {
    pub fn new(cfg: &VrchatAvatarConfig) -> Self {
        let mut param_names = Vec::new();
        for p in &cfg.parameters {
            param_names.push(p.name.clone());
        }
        Self {
            avatar_name: cfg.name.clone(),
            parameters: param_names,
        }
    }

    /// For demonstration, we just print them.
    pub fn print_menu(&self) {
        println!("======= TOGGLE MENU for AVATAR: {} =======", self.avatar_name);
        for (i, param_name) in self.parameters.iter().enumerate() {
            println!("  [{}] {}", i + 1, param_name);
        }
        println!("==========================================");
    }
}
