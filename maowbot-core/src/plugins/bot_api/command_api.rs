use async_trait::async_trait;
use uuid::Uuid;
use crate::Error;
use crate::models::{Command, CommandUsage};

/// A sub-trait for managing chat commands from external clients (TUI, GUI, etc.).
#[async_trait]
pub trait CommandApi: Send + Sync {
    async fn list_commands(&self, platform: &str) -> Result<Vec<Command>, Error>;
    async fn create_command(&self, platform: &str, command_name: &str, min_role: &str) -> Result<Command, Error>;
    async fn set_command_active(&self, command_id: Uuid, is_active: bool) -> Result<(), Error>;
    async fn update_command_role(&self, command_id: Uuid, new_role: &str) -> Result<(), Error>;
    async fn delete_command(&self, command_id: Uuid) -> Result<(), Error>;

    // For usage logs, you might provide queries:
    async fn get_usage_for_command(&self, command_id: Uuid, limit: i64) -> Result<Vec<CommandUsage>, Error>;
    async fn get_usage_for_user(&self, user_id: Uuid, limit: i64) -> Result<Vec<CommandUsage>, Error>;
}