use std::sync::Arc;
use chrono::Utc;
use uuid::Uuid;
use tracing::{info, warn, error};

use crate::Error;
use crate::models::{Command, CommandUsage};
use crate::repositories::{
    CommandRepository, CommandUsageRepository,
};
use crate::services::user_service::UserService;
use crate::models::User;

/// A service that processes “bang” commands like `!lurk`.
///
/// The typical usage flow:
/// - We parse an incoming chat line to see if it starts with '!' (or some custom prefix).
/// - If so, we strip the prefix, find the command in DB, check user roles, log usage,
///   and optionally perform custom logic.
pub struct CommandService {
    command_repo: Arc<dyn CommandRepository + Send + Sync>,
    usage_repo: Arc<dyn CommandUsageRepository + Send + Sync>,
    user_service: Arc<UserService>, // to check roles or fetch user
}

impl CommandService {
    pub fn new(
        command_repo: Arc<dyn CommandRepository + Send + Sync>,
        usage_repo: Arc<dyn CommandUsageRepository + Send + Sync>,
        user_service: Arc<UserService>,
    ) -> Self {
        Self {
            command_repo,
            usage_repo,
            user_service,
        }
    }

    /// Attempt to handle a chat line that might be a command.
    /// If it is a recognized command, perform checks and log usage.
    ///
    /// `user_roles` are the roles from the chat platform (like “mod”, “vip”), but we can also
    /// cross-reference DB roles. This example just uses the platform roles plus the DB-saved roles.
    pub async fn handle_chat_line(
        &self,
        platform: &str,
        channel: &str,
        user_id: Uuid,
        user_roles: &[String],
        message_text: &str,
    ) -> Result<bool, Error> {
        // check if it starts with '!'
        if !message_text.trim().starts_with('!') {
            return Ok(false);
        }

        // parse out the command name (e.g. "!lurk" => "lurk")
        let parts: Vec<&str> = message_text.trim().split_whitespace().collect();
        let cmd_part = parts[0].trim_start_matches('!');
        let args = if parts.len() > 1 {
            parts[1..].join(" ")
        } else {
            "".to_string()
        };

        // find the command in DB
        let cmd_opt = self.command_repo
            .get_command_by_name(platform, cmd_part)
            .await?;
        let cmd = match cmd_opt {
            Some(c) => c,
            None => {
                // unknown command
                return Ok(false);
            }
        };

        // is the command active?
        if !cmd.is_active {
            return Ok(false);
        }

        // check role requirement
        // (very simplified: check if user_roles contains cmd.min_role or if min_role == "everyone")
        if cmd.min_role.to_lowercase() != "everyone" {
            // user must have cmd.min_role
            if !user_roles.iter().any(|r| r.to_lowercase() == cmd.min_role.to_lowercase()) {
                // lacking the required role
                warn!("User lacks required role '{}' for command '{}'", cmd.min_role, cmd.command_name);
                return Ok(false);
            }
        }

        // log usage
        let usage = CommandUsage {
            usage_id: Uuid::new_v4(),
            command_id: cmd.command_id,
            user_id,
            used_at: Utc::now(),
            channel: channel.to_string(),
            usage_text: Some(args.clone()),
            metadata: None,
        };
        if let Err(e) = self.usage_repo.insert_usage(&usage).await {
            error!("Failed to insert command usage => {:?}", e);
        }

        // [Optional] perform custom logic for the command (omitted).
        // E.g. you might trigger some “bot says hello” logic.

        info!("Command '{}' used by user_id={} in channel='{}'", cmd.command_name, user_id, channel);
        Ok(true)
    }

    // Additional service methods:
    pub async fn create_command(&self, platform: &str, command_name: &str, min_role: &str) -> Result<Command, Error> {
        let now = Utc::now();
        let cmd = Command {
            command_id: Uuid::new_v4(),
            platform: platform.to_string(),
            command_name: command_name.to_string(),
            min_role: min_role.to_string(),
            is_active: true,
            created_at: now,
            updated_at: now,
        };
        self.command_repo.create_command(&cmd).await?;
        Ok(cmd)
    }

    pub async fn list_commands(&self, platform: &str) -> Result<Vec<Command>, Error> {
        self.command_repo.list_commands(platform).await
    }

    pub async fn update_command_role(&self, command_id: Uuid, new_role: &str) -> Result<(), Error> {
        if let Some(mut cmd) = self.command_repo.get_command_by_id(command_id).await? {
            cmd.min_role = new_role.to_string();
            cmd.updated_at = Utc::now();
            self.command_repo.update_command(&cmd).await?;
        }
        Ok(())
    }

    pub async fn set_command_active(&self, command_id: Uuid, is_active: bool) -> Result<(), Error> {
        if let Some(mut cmd) = self.command_repo.get_command_by_id(command_id).await? {
            cmd.is_active = is_active;
            cmd.updated_at = Utc::now();
            self.command_repo.update_command(&cmd).await?;
        }
        Ok(())
    }

    pub async fn delete_command(&self, command_id: Uuid) -> Result<(), Error> {
        self.command_repo.delete_command(command_id).await
    }
}
