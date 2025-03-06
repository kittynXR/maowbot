//! plugins/bot_api/vrchat_api.rs
//!
//! Sub-trait for VRChat-related queries (world, avatar, instance).

use crate::Error;
use async_trait::async_trait;

/// Basic fields representing a VRChat world.
#[derive(Debug)]
pub struct VRChatWorldBasic {
    pub name: String,
    pub author_name: String,
    pub updated_at: String,
    pub created_at: String,
    pub capacity: u32,

    /// New: textual "release status" from VRChat (e.g. "public", "private", "hidden", "all ...", "communityLabs").
    pub release_status: String,

    /// Optional description if present
    pub description: String,
}

/// Basic fields representing a VRChat instance.
#[derive(Debug)]
pub struct VRChatInstanceBasic {
    pub world_id: Option<String>,
    pub instance_id: Option<String>,
    pub location: Option<String>,
}

/// Basic fields representing a VRChat avatar.
#[derive(Debug)]
pub struct VRChatAvatarBasic {
    pub avatar_id: String,
    pub avatar_name: String,
}

/// VRChat-specific trait.
#[async_trait]
pub trait VrchatApi: Send + Sync {
    /// Returns info about the user’s current VRChat world.
    async fn vrchat_get_current_world(&self, account_name: &str) -> Result<VRChatWorldBasic, Error>;

    /// Returns info about the user’s current VRChat avatar.
    async fn vrchat_get_current_avatar(&self, account_name: &str) -> Result<VRChatAvatarBasic, Error>;

    /// Changes the user’s current avatar to the given `new_avatar_id`.
    async fn vrchat_change_avatar(&self, account_name: &str, new_avatar_id: &str) -> Result<(), Error>;

    /// Returns info about the user’s current instance (world_id + instance_id).
    async fn vrchat_get_current_instance(&self, account_name: &str) -> Result<VRChatInstanceBasic, Error>;
}
