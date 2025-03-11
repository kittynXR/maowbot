// maowbot-core/src/plugins/bot_api/drip_api.rs
//
// Sub-trait for handling all "drip" related commands and logic.
// This includes ignoring/stripping prefixes, listing avatars,
// creating fits, adding/deleting param values, etc.

use async_trait::async_trait;
use crate::Error;

/// Represents a summary or minimal info about an avatar from the drip system.
#[derive(Debug)]
pub struct DripAvatarSummary {
    pub local_name: Option<String>,
    pub vrchat_avatar_id: String,
    pub vrchat_avatar_name: String,
}

/// Information about a single outfit.
#[derive(Debug)]
pub struct DripFitSummary {
    pub fit_name: String,
    pub param_count: usize,
}

/// Data about a prop (basic name, etc.). We might expand as needed.
#[derive(Debug)]
pub struct DripPropSummary {
    pub prop_name: String,
    pub param_count: usize,
}

/// Trait providing "drip" commands.
/// Each method below corresponds to a subcommand or action, e.g.
///   drip set i/ignore <prefix>
#[async_trait]
pub trait DripApi: Send + Sync {
    /// `drip set` => show settable parameters (non-outfit). This might just list rules, etc.
    async fn drip_show_settable(&self) -> Result<String, Error>;

    /// `drip set i/ignore <prefix>`
    async fn drip_set_ignore_prefix(&self, prefix: &str) -> Result<String, Error>;

    /// `drip set s/strip <prefix>`
    async fn drip_set_strip_prefix(&self, prefix: &str) -> Result<String, Error>;

    /// `drip set name <name>` => rename local avatar
    async fn drip_set_avatar_name(&self, new_name: &str) -> Result<String, Error>;

    /// `drip list` => list stored avatars in database
    async fn drip_list_avatars(&self) -> Result<Vec<DripAvatarSummary>, Error>;

    /// `drip fit new <name>` => create new outfit for current avatar
    async fn drip_fit_new(&self, fit_name: &str) -> Result<String, Error>;

    /// `drip fit add <name> <param> <value>`
    async fn drip_fit_add_param(&self, fit_name: &str, param_name: &str, param_value: &str) -> Result<String, Error>;

    /// `drip fit del <name> <param> <value>`
    async fn drip_fit_del_param(&self, fit_name: &str, param_name: &str, param_value: &str) -> Result<String, Error>;

    /// `drip fit w/wear <name>` => sets the parameters for that fit, outputs any missing
    async fn drip_fit_wear(&self, fit_name: &str) -> Result<String, Error>;

    /// `drip props add <prop_name> <param> <value>`
    async fn drip_props_add(&self, prop_name: &str, param_name: &str, param_value: &str) -> Result<String, Error>;

    /// `drip props del <prop_name> <param> <value>`
    async fn drip_props_del(&self, prop_name: &str, param_name: &str, param_value: &str) -> Result<String, Error>;

    /// `drip props timer <prop_name> <timer_data>`
    async fn drip_props_timer(&self, prop_name: &str, timer_data: &str) -> Result<String, Error>;
}
