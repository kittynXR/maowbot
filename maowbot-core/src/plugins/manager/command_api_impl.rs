use std::sync::Arc;
use async_trait::async_trait;
use uuid::Uuid;
use crate::Error;
use crate::models::{Command, CommandUsage};
use crate::plugins::bot_api::command_api::CommandApi;
use crate::plugins::manager::core::PluginManager;
use crate::repositories::{
    CommandUsageRepository,
};

#[async_trait]
impl CommandApi for PluginManager {
    async fn list_commands(&self, platform: &str) -> Result<Vec<Command>, Error> {
        let cmd_service = self.resolve_command_service()?;
        cmd_service.list_commands(platform).await
    }

    async fn create_command(&self, platform: &str, command_name: &str, min_role: &str) -> Result<Command, Error> {
        let cmd_service = self.resolve_command_service()?;
        cmd_service.create_command(platform, command_name, min_role).await
    }

    async fn set_command_active(&self, command_id: Uuid, is_active: bool) -> Result<(), Error> {
        let cmd_service = self.resolve_command_service()?;
        cmd_service.set_command_active(command_id, is_active).await
    }

    async fn update_command_role(&self, command_id: Uuid, new_role: &str) -> Result<(), Error> {
        let cmd_service = self.resolve_command_service()?;
        cmd_service.update_command_role(command_id, new_role).await
    }

    async fn delete_command(&self, command_id: Uuid) -> Result<(), Error> {
        let cmd_service = self.resolve_command_service()?;
        cmd_service.delete_command(command_id).await
    }

    // usage logs
    async fn get_usage_for_command(&self, command_id: Uuid, limit: i64) -> Result<Vec<CommandUsage>, Error> {
        let usage_repo = match &self.command_usage_repo {
            repo => repo.clone(),
        };
        usage_repo.list_usage_for_command(command_id, limit).await
    }

    async fn get_usage_for_user(&self, user_id: Uuid, limit: i64) -> Result<Vec<CommandUsage>, Error> {
        let usage_repo = match &self.command_usage_repo {
            repo => repo.clone(),
        };
        usage_repo.list_usage_for_user(user_id, limit).await
    }
}

// Helper method on PluginManager to get CommandService
impl PluginManager {
    pub fn resolve_command_service(&self) -> Result<Arc<crate::services::CommandService>, Error> {
        match &self.command_service {
            svc => Ok(svc.clone()),
        }
    }
}